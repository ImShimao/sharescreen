//! Session miroir Android : serveur scrcpy sur la tablette, sockets vidéo, audio et contrôle.

use std::collections::VecDeque;
use std::net::TcpListener;
use std::process::Stdio;
use std::sync::{Arc, Mutex as StdMutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::Deserialize;
use serde_json::json;
use tauri::ipc::{Channel, InvokeResponseBody};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::{tcp::OwnedWriteHalf, TcpStream};
use tokio::process::Child;
use tokio::sync::{mpsc, oneshot};
use tokio::task::JoinHandle;
use tokio::time::{sleep, timeout};

use crate::adb;
use crate::devices::server_failure_hint;
use crate::events;
use crate::paths;
use crate::protocol::{
    h264_packet_to_chunk, parse_h264_config, parse_opus_config, ParserOptions, StreamEvent, StreamParser, CODEC_H264,
    CODEC_OPUS, SCRCPY_VERSION,
};

const REMOTE_SERVER: &str = "/data/local/tmp/scrcpy-server.jar";
const CONNECT_ATTEMPTS: u32 = 80;
const CONNECT_DELAY: Duration = Duration::from_millis(120);
const READY_TIMEOUT: Duration = Duration::from_secs(20);
const RECENT_LINES: usize = 12;

/// Canal binaire vers l'interface (voir events.rs pour le format).
pub type Emitter = Channel<InvokeResponseBody>;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AndroidOptions {
    pub serial: String,
    pub max_size: u32,
    pub bit_rate: u32,
    pub max_fps: u32,
    pub audio: bool,
    pub stay_awake: bool,
}

type Recent = Arc<StdMutex<VecDeque<String>>>;

#[derive(Default)]
struct Shared {
    width: u32,
    height: u32,
    device_name: String,
}

fn emit(emitter: &Emitter, bytes: Vec<u8>) {
    let _ = emitter.send(InvokeResponseBody::Raw(bytes));
}

fn hint(recent: &Recent) -> String {
    let lines: Vec<String> = recent.lock().map(|l| l.iter().cloned().collect()).unwrap_or_default();
    server_failure_hint(&lines)
}

fn with_hint(message: &str, recent: &Recent) -> String {
    let detail = hint(recent);
    if detail.is_empty() {
        message.to_string()
    } else {
        format!("{message} {detail}")
    }
}

/// Session active : garde les tâches, le serveur sur la tablette et le canal de contrôle.
pub struct AndroidSession {
    serial: String,
    port: u16,
    server: Child,
    tasks: Vec<JoinHandle<()>>,
    control: mpsc::UnboundedSender<Vec<u8>>,
    shared: Arc<StdMutex<Shared>>,
}

impl AndroidSession {
    /// Taille de l'image vidéo courante, pour convertir les coordonnées de l'interface.
    pub fn screen_size(&self) -> (u32, u32) {
        self.shared.lock().map(|s| (s.width, s.height)).unwrap_or((0, 0))
    }

    /// Envoie un message de contrôle déjà encodé.
    pub fn send(&self, message: Vec<u8>) {
        let _ = self.control.send(message);
    }

    pub async fn start(options: AndroidOptions, emitter: Emitter) -> Result<Self, String> {
        let serial = options.serial.clone();
        let jar = paths::bin("scrcpy-server.jar");
        if !jar.exists() {
            return Err("scrcpy-server.jar est absent du dossier bin.".into());
        }
        adb::push(&serial, &jar.to_string_lossy(), REMOTE_SERVER).await?;

        let scid = new_scid();
        let port = free_port()?;
        adb::forward(&serial, port, &format!("scrcpy_{scid}")).await?;

        let recent: Recent = Arc::new(StdMutex::new(VecDeque::new()));
        let mut server = adb::spawn_shell(&serial, &server_command(&options, &scid))?;
        watch_output(&mut server, recent.clone());

        // Ordre imposé par le serveur : vidéo (octet factice), puis audio, puis contrôle.
        let (video, first) = match open_video(port).await {
            Ok(v) => v,
            Err(e) => {
                teardown(&mut server, &serial, port).await;
                return Err(with_hint(&e, &recent));
            }
        };
        let audio = if options.audio {
            match connect_plain(port).await {
                Ok(s) => Some(s),
                Err(e) => {
                    teardown(&mut server, &serial, port).await;
                    return Err(with_hint(&e, &recent));
                }
            }
        } else {
            None
        };
        let control = match connect_plain(port).await {
            Ok(s) => s,
            Err(e) => {
                teardown(&mut server, &serial, port).await;
                return Err(with_hint(&e, &recent));
            }
        };

        let shared = Arc::new(StdMutex::new(Shared::default()));
        let (ready_tx, ready_rx) = oneshot::channel();
        let (control_tx, control_rx) = mpsc::unbounded_channel::<Vec<u8>>();
        let (mut control_read, control_write) = control.into_split();

        let mut tasks = vec![tokio::spawn(video_task(
            video,
            first,
            emitter.clone(),
            shared.clone(),
            ready_tx,
            recent.clone(),
        ))];
        if let Some(audio) = audio {
            tasks.push(tokio::spawn(audio_task(audio, emitter.clone())));
        }
        tasks.push(tokio::spawn(async move {
            let mut sink = [0u8; 1024];
            while let Ok(n) = control_read.read(&mut sink).await {
                if n == 0 {
                    break;
                }
            }
        }));
        tasks.push(tokio::spawn(writer_task(control_write, control_rx)));

        match timeout(READY_TIMEOUT, ready_rx).await {
            Ok(Ok(())) => {}
            Ok(Err(_)) => {
                abort_all(&tasks);
                teardown(&mut server, &serial, port).await;
                return Err(with_hint("La tablette a fermé l'image avant de démarrer.", &recent));
            }
            Err(_) => {
                abort_all(&tasks);
                teardown(&mut server, &serial, port).await;
                return Err(with_hint("La tablette n'envoie pas d'image. Vérifiez le débogage USB.", &recent));
            }
        }

        let (width, height, device_name) = {
            let s = shared.lock().map_err(|_| "État de session indisponible.")?;
            (s.width, s.height, s.device_name.clone())
        };
        emit(
            &emitter,
            events::json(
                events::READY,
                &json!({ "width": width, "height": height, "deviceName": device_name, "audio": options.audio }),
            ),
        );

        Ok(Self { serial, port, server, tasks, control: control_tx, shared })
    }

    pub async fn stop(mut self) {
        abort_all(&self.tasks);
        self.tasks.clear();
        let _ = self.server.start_kill();
        adb::remove_forward(&self.serial, self.port).await;
    }
}

impl Drop for AndroidSession {
    fn drop(&mut self) {
        abort_all(&self.tasks);
        let _ = self.server.start_kill();
        // Retire le transfert adb sans attendre : la commande est courte et indépendante.
        let mut cmd = std::process::Command::new(paths::bin("adb.exe"));
        cmd.args(["-s", &self.serial, "forward", "--remove", &format!("tcp:{}", self.port)]);
        cmd.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            cmd.creation_flags(0x0800_0000); // pas de fenêtre console
        }
        let _ = cmd.spawn();
    }
}

fn abort_all(tasks: &[JoinHandle<()>]) {
    for t in tasks {
        t.abort();
    }
}

async fn teardown(server: &mut Child, serial: &str, port: u16) {
    let _ = server.start_kill();
    adb::remove_forward(serial, port).await;
}

fn new_scid() -> String {
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_nanos() as u64).unwrap_or(1);
    format!("{:08x}", (nanos % 0x7fff_fffe) as u32 + 1)
}

fn free_port() -> Result<u16, String> {
    let listener = TcpListener::bind("127.0.0.1:0").map_err(|e| format!("Port local indisponible : {e}"))?;
    let port = listener.local_addr().map_err(|e| e.to_string())?.port();
    Ok(port)
}

fn server_command(options: &AndroidOptions, scid: &str) -> Vec<String> {
    vec![
        format!("CLASSPATH={REMOTE_SERVER}"),
        "app_process".into(),
        "/".into(),
        "com.genymobile.scrcpy.Server".into(),
        SCRCPY_VERSION.into(),
        format!("scid={scid}"),
        "log_level=info".into(),
        "tunnel_forward=true".into(),
        format!("video_bit_rate={}", options.bit_rate),
        format!("max_size={}", options.max_size),
        format!("max_fps={}", options.max_fps),
        format!("audio={}", options.audio),
        format!("stay_awake={}", options.stay_awake),
    ]
}

/// Conserve les dernières lignes du serveur : elles expliquent un refus (encodeur, sécurité, version).
fn watch_output(server: &mut Child, recent: Recent) {
    let push_lines = |reader: Box<dyn tokio::io::AsyncRead + Unpin + Send>| {
        let recent = recent.clone();
        tokio::spawn(async move {
            let mut lines = BufReader::new(reader).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                if let Ok(mut q) = recent.lock() {
                    q.push_back(line);
                    while q.len() > RECENT_LINES {
                        q.pop_front();
                    }
                }
            }
        });
    };
    if let Some(out) = server.stdout.take() {
        push_lines(Box::new(out));
    }
    if let Some(err) = server.stderr.take() {
        push_lines(Box::new(err));
    }
}

async fn connect_once(port: u16) -> Option<TcpStream> {
    TcpStream::connect(("127.0.0.1", port)).await.ok()
}

/// Connexion vidéo : le serveur envoie son octet factice une fois la connexion acceptée.
/// adb ferme la connexion tant que le serveur n'écoute pas encore, on réessaie alors.
async fn open_video(port: u16) -> Result<(TcpStream, Vec<u8>), String> {
    for _ in 0..CONNECT_ATTEMPTS {
        if let Some(mut stream) = connect_once(port).await {
            let mut buf = vec![0u8; 64 * 1024];
            if let Ok(n) = stream.read(&mut buf).await {
                if n > 0 {
                    buf.truncate(n);
                    return Ok((stream, buf));
                }
            }
        }
        sleep(CONNECT_DELAY).await;
    }
    Err("La tablette ne répond pas. Vérifiez le débogage USB et l'autorisation sur l'écran.".into())
}

/// Connexion simple, pour les sockets que le serveur ne lit qu'après l'acceptation du précédent.
async fn connect_plain(port: u16) -> Result<TcpStream, String> {
    for _ in 0..CONNECT_ATTEMPTS {
        if let Some(stream) = connect_once(port).await {
            return Ok(stream);
        }
        sleep(CONNECT_DELAY).await;
    }
    Err("Connexion au serveur ShareScreen impossible.".into())
}

/// Traite le flux vidéo : configuration, images, taille et première session.
struct VideoPump {
    parser: StreamParser,
    emitter: Emitter,
    shared: Arc<StdMutex<Shared>>,
    ready: Option<oneshot::Sender<()>>,
    h264: Option<crate::protocol::H264Config>,
}

impl VideoPump {
    fn feed(&mut self, chunk: &[u8]) -> Result<(), String> {
        for event in self.parser.feed(chunk) {
            match event {
                StreamEvent::Device(name) => {
                    if let Ok(mut s) = self.shared.lock() {
                        s.device_name = name;
                    }
                }
                StreamEvent::Codec(codec) => {
                    if codec != CODEC_H264 {
                        return Err("Codec vidéo non pris en charge par la tablette.".into());
                    }
                }
                StreamEvent::Session { width, height, .. } => {
                    let changed = match self.shared.lock() {
                        Ok(mut s) => {
                            let changed = s.width != width || s.height != height;
                            s.width = width;
                            s.height = height;
                            changed
                        }
                        Err(_) => false,
                    };
                    if let Some(tx) = self.ready.take() {
                        let _ = tx.send(());
                    } else if changed {
                        emit(&self.emitter, events::json(events::SIZE, &json!({ "width": width, "height": height })));
                    }
                }
                StreamEvent::Packet { config: true, data, .. } => {
                    let cfg = parse_h264_config(&data)?;
                    emit(&self.emitter, events::video_config(&cfg.codec, &cfg.description));
                    self.h264 = Some(cfg);
                }
                StreamEvent::Packet { config: false, key_frame, pts, data } => {
                    if let Some(cfg) = &self.h264 {
                        let chunk = h264_packet_to_chunk(&data, key_frame, Some(cfg));
                        emit(&self.emitter, events::timed_frame(events::VIDEO_FRAME, Some(key_frame), pts, &chunk));
                    }
                }
            }
        }
        Ok(())
    }
}

async fn video_task(
    mut stream: TcpStream,
    first: Vec<u8>,
    emitter: Emitter,
    shared: Arc<StdMutex<Shared>>,
    ready: oneshot::Sender<()>,
    recent: Recent,
) {
    let mut pump = VideoPump {
        parser: StreamParser::new(ParserOptions { dummy_byte: true, device_meta: true }),
        emitter: emitter.clone(),
        shared,
        ready: Some(ready),
        h264: None,
    };
    let base: String = match pump.feed(&first) {
        Err(e) => e,
        Ok(()) => loop {
            let mut buf = vec![0u8; 64 * 1024];
            match stream.read(&mut buf).await {
                Ok(0) | Err(_) => break "La connexion vidéo a été fermée.".to_string(),
                Ok(n) => {
                    if let Err(e) = pump.feed(&buf[..n]) {
                        break e;
                    }
                }
            }
        },
    };
    emit(&emitter, events::raw(events::CLOSED, with_hint(&base, &recent).as_bytes()));
}

/// Traite le flux audio Opus. S'arrête si l'appareil ne peut pas capturer le son.
async fn audio_task(mut stream: TcpStream, emitter: Emitter) {
    let mut parser = StreamParser::new(ParserOptions { dummy_byte: false, device_meta: false });
    let mut configured = false;
    let mut buf = vec![0u8; 16 * 1024];
    'read: loop {
        let n = match stream.read(&mut buf).await {
            Ok(0) | Err(_) => break,
            Ok(n) => n,
        };
        for event in parser.feed(&buf[..n]) {
            match event {
                StreamEvent::Codec(codec) => {
                    // Codec 0 (désactivé) ou 1 (erreur) : la vidéo continue sans son.
                    if codec != CODEC_OPUS {
                        break 'read;
                    }
                }
                StreamEvent::Packet { config: true, data, .. } => match parse_opus_config(&data) {
                    Ok(desc) => {
                        emit(&emitter, events::raw(events::AUDIO_CONFIG, &desc));
                        configured = true;
                    }
                    Err(_) => break 'read,
                },
                StreamEvent::Packet { config: false, pts, data, .. } => {
                    if configured {
                        emit(&emitter, events::timed_frame(events::AUDIO_FRAME, None, pts, &data));
                    }
                }
                _ => {}
            }
        }
    }
}

async fn writer_task(mut write: OwnedWriteHalf, mut rx: mpsc::UnboundedReceiver<Vec<u8>>) {
    while let Some(msg) = rx.recv().await {
        if write.write_all(&msg).await.is_err() {
            break;
        }
    }
}
