//! iPhone : go-ios (ios.exe) pour la détection, la préparation, l'affichage MJPEG et le tunnel.
//! Le contrôle passe par WebDriverAgent (port 8100), à installer une fois sur l'appareil.

use std::collections::VecDeque;
use std::process::Stdio;
use std::sync::Arc;
use std::sync::Mutex as StdMutex;
use std::time::Duration;

#[cfg(windows)]
use std::os::windows::process::CommandExt;

use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::oneshot;
use tokio::task::JoinHandle;
use tokio::time::{sleep, timeout};

use crate::devices::{ios_has_developer_mode, ios_needs_tunnel};
use crate::events;
use crate::android::Emitter;
use crate::mjpeg::extract_jpeg_frames;
use crate::models::{DeviceInfo, IosStatus};
use crate::paths;

const WDA_URL: &str = "http://127.0.0.1:8100";
const CREATE_NO_WINDOW: u32 = 0x0800_0000;
const DETAIL_LINES: usize = 8;

fn exe() -> std::path::PathBuf {
    paths::bin("ios.exe")
}

fn command(args: &[&str]) -> Command {
    let mut cmd = Command::new(exe());
    cmd.args(args).stdin(Stdio::null());
    #[cfg(windows)]
    cmd.as_std_mut().creation_flags(CREATE_NO_WINDOW);
    cmd
}

async fn run(args: &[&str], seconds: u64) -> Result<String, String> {
    let out = timeout(Duration::from_secs(seconds), command(args).output())
        .await
        .map_err(|_| "ios.exe ne répond pas (délai dépassé).".to_string())?
        .map_err(|e| format!("ios.exe introuvable : {e}"))?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    } else {
        let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
        Err(if err.is_empty() { String::from_utf8_lossy(&out.stdout).trim().to_string() } else { err })
    }
}

/// UDID des iPhone branchés (`ios list` -> {"deviceList": [...]}).
pub async fn list_udids() -> Result<Vec<String>, String> {
    let json: Value = serde_json::from_str(&run(&["list"], 15).await?).unwrap_or(Value::Null);
    Ok(json["deviceList"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|e| match e {
                    Value::String(s) => Some(s.clone()),
                    _ => e["Properties"]["SerialNumber"].as_str().map(str::to_string),
                })
                .collect()
        })
        .unwrap_or_default())
}

#[derive(Debug, Clone, PartialEq)]
pub struct IosInfo {
    pub name: String,
    pub version: String,
}

/// Nom et version via `ios info` (nécessite l'appairage).
pub async fn info(udid: &str) -> Result<IosInfo, String> {
    let json: Value = serde_json::from_str(&run(&["--udid", udid, "info"], 20).await?)
        .map_err(|e| format!("Réponse inattendue d'ios info : {e}"))?;
    Ok(IosInfo {
        name: json["DeviceName"].as_str().unwrap_or("iPhone").to_string(),
        version: json["ProductVersion"].as_str().unwrap_or("").to_string(),
    })
}

pub async fn list_devices() -> Vec<DeviceInfo> {
    let udids = list_udids().await.unwrap_or_default();
    let mut devices = Vec::new();
    for udid in udids {
        match info(&udid).await {
            Ok(i) => devices.push(DeviceInfo {
                kind: "ios".into(),
                id: udid,
                name: i.name,
                state: "ready".into(),
                detail: Some(format!(
                    "iOS {}{}",
                    i.version,
                    if ios_needs_tunnel(&i.version) { " · tunnel requis" } else { "" }
                )),
                hint: None,
            }),
            Err(_) => devices.push(DeviceInfo {
                kind: "ios".into(),
                id: udid,
                name: "iPhone".into(),
                state: "untrusted".into(),
                detail: Some("Faites confiance à cet ordinateur sur l'iPhone, puis rebranchez-le.".into()),
                hint: None,
            }),
        }
    }
    devices
}

async fn developer_mode(udid: &str) -> Option<bool> {
    let text = run(&["--udid", udid, "devmode", "get"], 20).await.ok()?;
    serde_json::from_str::<Value>(&text).ok()?["DeveloperModeEnabled"].as_bool()
}

async fn wda_available() -> bool {
    let Ok(client) = reqwest::Client::builder().timeout(Duration::from_millis(1500)).build() else {
        return false;
    };
    client.get(format!("{WDA_URL}/status")).send().await.map(|r| r.status().is_success()).unwrap_or(false)
}

/// État complet pour la checklist de l'interface.
pub async fn status(udid: Option<String>) -> IosStatus {
    let wda = wda_available().await;
    let mut s = IosStatus {
        ios: exe().exists(),
        version: String::new(),
        paired: false,
        developer_mode: None,
        tunnel_required: false,
        tunnel: false,
        wda,
        message: String::new(),
    };
    if !s.ios {
        s.message = "ios.exe est absent du dossier bin.".into();
        return s;
    }
    let Some(udid) = udid else {
        s.message = "Branchez un iPhone.".into();
        return s;
    };
    let Ok(info) = info(&udid).await else {
        s.message = "Faites confiance à cet ordinateur sur l'iPhone, puis rebranchez-le.".into();
        return s;
    };
    s.version = info.version.clone();
    s.paired = true;
    s.tunnel_required = ios_needs_tunnel(&info.version);
    s.developer_mode = if ios_has_developer_mode(&info.version) { developer_mode(&udid).await } else { None };
    let tunnels = run(&["tunnel", "ls"], 5).await.unwrap_or_default();
    s.tunnel = !s.tunnel_required || tunnels.contains(&udid);
    s.message = if s.developer_mode == Some(false) {
        "Activez le Mode développeur sur l'iPhone.".into()
    } else if s.tunnel_required && !s.tunnel {
        "Activez le tunnel iOS (droits administrateur).".into()
    } else if !wda {
        "Affichage disponible. Le contrôle exige WebDriverAgent.".into()
    } else {
        "Prêt.".into()
    };
    s
}

/// Demande l'appairage : l'iPhone affiche alors « Faire confiance ».
pub async fn pair(udid: &str) -> Result<(), String> {
    run(&["--udid", udid, "pair"], 30)
        .await
        .map(|_| ())
        .map_err(|e| format!("Appairage impossible. Déverrouillez l'iPhone, acceptez « Faire confiance » puis réessayez. {e}"))
}

/// Révèle l'option Mode développeur dans les réglages (iOS 16 et plus).
pub async fn reveal_developer_mode(udid: &str) -> Result<(), String> {
    run(&["--udid", udid, "devmode", "reveal"], 30)
        .await
        .map(|_| ())
        .map_err(|e| format!("Impossible de révéler le Mode développeur. {e}"))
}

/// Vérifie que l'iPhone peut être affiché, puis monte l'image développeur (téléchargée une fois).
pub async fn prepare(udid: &str) -> Result<(), String> {
    let s = status(Some(udid.to_string())).await;
    if !s.paired {
        return Err("iPhone non appairé. Acceptez « Faire confiance » sur l'iPhone.".into());
    }
    if s.developer_mode == Some(false) {
        return Err("Mode développeur désactivé. Activez-le dans Réglages > Confidentialité et sécurité.".into());
    }
    if s.tunnel_required && !s.tunnel {
        return Err("Tunnel iOS inactif. Activez-le avec le bouton ci-dessous.".into());
    }
    run(&["--udid", udid, "image", "auto"], 300)
        .await
        .map(|_| ())
        .map_err(|e| explain_image_error(&e))
}

/// Traduit la sortie brute de go-ios (lignes JSON) en message lisible pour l'utilisateur.
pub fn explain_image_error(raw: &str) -> String {
    let detail = raw
        .lines()
        .rev()
        .find_map(|line| {
            let v: Value = serde_json::from_str(line).ok()?;
            (v["level"] == "ERROR").then(|| v["err"].as_str().unwrap_or_else(|| v["msg"].as_str().unwrap_or("")).to_string())
        })
        .unwrap_or_else(|| raw.trim().to_string());
    if detail.contains("certificate signed by unknown authority") || detail.contains("failed to verify certificate") {
        "Windows ne fait pas confiance au certificat d'Apple : l'image développeur ne peut pas être signée. Il faut installer les certificats Apple (intermédiaire « Apple Server Authentication CA » et racine « Apple Root CA ») dans Windows.".into()
    } else if detail.contains("failed to send request") {
        "Apple est injoignable depuis ce PC (connexion Internet ou pare-feu).".into()
    } else {
        format!("Image développeur indisponible : {detail}")
    }
}

/// Quote une chaîne pour PowerShell (apostrophes doublées).
fn ps_str(p: &std::path::Path) -> String {
    format!("'{}'", p.to_string_lossy().replace('\'', "''"))
}

/// Lance le tunnel iOS 17+ : copie wintun.dll dans System32 si besoin, puis démarre le tunnel.
/// Tout passe par un seul script en élévation : une seule fenêtre UAC.
pub async fn start_tunnel_elevated() -> Result<(), String> {
    let system32 = std::path::PathBuf::from(std::env::var("SystemRoot").unwrap_or_else(|_| "C:\\Windows".into())).join("System32");
    let needs_wintun = !system32.join("wintun.dll").exists();
    let wintun_src = paths::bin("wintun.dll");
    if needs_wintun && !wintun_src.exists() {
        return Err("wintun.dll est absent de l'application : réinstallez ShareScreen.".into());
    }
    let mut script = String::from("$ErrorActionPreference = 'Stop'\n");
    if needs_wintun {
        script.push_str(&format!("Copy-Item -LiteralPath {} -Destination {} -Force\n", ps_str(&wintun_src), ps_str(&system32.join("wintun.dll"))));
    }
    script.push_str(&format!("Start-Process -FilePath {} -ArgumentList 'tunnel','start' -WindowStyle Hidden\n", ps_str(&exe())));
    let script_file = std::env::temp_dir().join("sharescreen-tunnel.ps1");
    std::fs::write(&script_file, script).map_err(|e| format!("Écriture du script impossible : {e}"))?;
    let launcher = format!(
        "$p = Start-Process -FilePath powershell.exe -Verb RunAs -WindowStyle Hidden -Wait -PassThru -ArgumentList '-NoProfile','-ExecutionPolicy','Bypass','-File',{}; exit $p.ExitCode",
        ps_str(&script_file)
    );
    let mut cmd = Command::new("powershell.exe");
    cmd.args(["-NoProfile", "-NonInteractive", "-Command", &launcher]).stdin(Stdio::null());
    #[cfg(windows)]
    cmd.as_std_mut().creation_flags(CREATE_NO_WINDOW);
    let out = cmd.output().await.map_err(|e| format!("PowerShell indisponible : {e}"))?;
    if out.status.success() {
        Ok(())
    } else {
        Err("Le tunnel n'a pas pu être lancé : droits administrateur refusés ou copie de wintun.dll impossible.".into())
    }
}

/// Port du serveur MJPEG de go-ios (valeur par défaut de `ios screenshot --stream`).
const STREAM_PORT: u16 = 3333;

/// Affichage de l'iPhone : `ios screenshot --stream` (MJPEG local), relayé à l'interface.
pub struct IosStream {
    child: Child,
    task: JoinHandle<()>,
}

impl IosStream {
    pub async fn start(udid: String, emitter: Emitter) -> Result<Self, String> {
        if !exe().exists() {
            return Err("ios.exe est absent du dossier bin.".into());
        }
        // go-ios ignore --port et utilise son port par défaut : on s'y connecte directement.
        let port = STREAM_PORT;
        let mut cmd = Command::new(exe());
        cmd.args(["--udid", &udid, "screenshot", "--stream"]);
        cmd.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::piped()).kill_on_drop(true);
        #[cfg(windows)]
        cmd.as_std_mut().creation_flags(CREATE_NO_WINDOW);
        let mut child = cmd.spawn().map_err(|e| format!("ios.exe n'a pas démarré : {e}"))?;

        let recent = Arc::new(StdMutex::new(VecDeque::<String>::new()));
        if let Some(err) = child.stderr.take() {
            let recent = recent.clone();
            tokio::spawn(async move {
                let mut lines = BufReader::new(err).lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    if let Ok(mut q) = recent.lock() {
                        q.push_back(line);
                        while q.len() > DETAIL_LINES {
                            q.pop_front();
                        }
                    }
                }
            });
        }

        let (ready_tx, ready_rx) = oneshot::channel::<Result<(), String>>();
        let task = tokio::spawn(stream_task(port, emitter, ready_tx));
        match timeout(Duration::from_secs(20), ready_rx).await {
            Ok(Ok(Ok(()))) => Ok(Self { child, task }),
            other => {
                task.abort();
                let _ = child.start_kill();
                let base = match other {
                    Ok(Ok(Err(e))) => e,
                    _ => "Le flux iPhone n'a pas démarré. Vérifiez le Mode développeur et le tunnel.".to_string(),
                };
                let detail = recent.lock().map(|q| q.iter().last().cloned().unwrap_or_default()).unwrap_or_default();
                Err(if detail.is_empty() { base } else { format!("{base} Détail : {detail}") })
            }
        }
    }

    pub fn stop(mut self) {
        self.task.abort();
        let _ = self.child.start_kill();
    }
}

async fn stream_task(port: u16, emitter: Emitter, ready: oneshot::Sender<Result<(), String>>) {
    let client = reqwest::Client::new();
    let url = format!("http://127.0.0.1:{port}/");
    let mut response = None;
    for _ in 0..60 {
        match client.get(&url).send().await {
            Ok(r) => {
                response = Some(r);
                break;
            }
            Err(_) => sleep(Duration::from_millis(250)).await,
        }
    }
    let Some(mut res) = response else {
        let _ = ready.send(Err("Le flux iPhone n'a pas démarré.".into()));
        return;
    };
    let _ = ready.send(Ok(()));

    let mut buffer: Vec<u8> = Vec::new();
    while let Ok(Some(chunk)) = res.chunk().await {
        buffer.extend_from_slice(&chunk);
        let (frames, consumed) = {
            let (frames, consumed) = extract_jpeg_frames(&buffer);
            (frames.iter().map(|f| f.to_vec()).collect::<Vec<_>>(), consumed)
        };
        buffer.drain(..consumed);
        for frame in frames {
            let _ = emitter.send(tauri::ipc::InvokeResponseBody::Raw(events::raw(events::IOS_FRAME, &frame)));
        }
    }
    let _ = emitter.send(tauri::ipc::InvokeResponseBody::Raw(events::raw(
        events::CLOSED,
        "Flux iPhone interrompu.".as_bytes(),
    )));
}

/// Contrôle via WebDriverAgent : session créée à la demande, coordonnées normalisées (0..1).
pub struct Wda {
    client: reqwest::Client,
    session: Option<String>,
    size: Option<(f64, f64)>,
}

impl Default for Wda {
    fn default() -> Self {
        Self::new()
    }
}

impl Wda {
    pub fn new() -> Self {
        let client = reqwest::Client::builder().timeout(Duration::from_secs(5)).build().unwrap_or_default();
        Self { client, session: None, size: None }
    }

    async fn session_id(&mut self) -> Result<String, String> {
        if let Some(id) = &self.session {
            return Ok(id.clone());
        }
        let res = self
            .client
            .post(format!("{WDA_URL}/session"))
            .json(&json!({ "capabilities": {} }))
            .send()
            .await
            .map_err(|_| "WebDriverAgent injoignable. Installez-le sur l'iPhone.".to_string())?;
        let v: Value = res.json().await.map_err(|e| e.to_string())?;
        let id = v["sessionId"]
            .as_str()
            .or_else(|| v["value"]["sessionId"].as_str())
            .ok_or("Session WebDriverAgent impossible.")?
            .to_string();
        self.session = Some(id.clone());
        Ok(id)
    }

    async fn post(&mut self, path: &str, body: Option<Value>) -> Result<(), String> {
        let sid = self.session_id().await?;
        let mut req = self.client.post(format!("{WDA_URL}/session/{sid}{path}"));
        if let Some(b) = body {
            req = req.json(&b);
        }
        let res = req.send().await.map_err(|e| e.to_string())?;
        let status = res.status();
        if status == reqwest::StatusCode::NOT_FOUND || status == reqwest::StatusCode::BAD_REQUEST {
            self.session = None; // session expirée : recréée au prochain appel
        }
        if !status.is_success() {
            return Err(format!("WebDriverAgent a refusé la commande ({}).", status.as_u16()));
        }
        Ok(())
    }

    /// Taille de l'écran en points iOS : les coordonnées WDA sont en points, pas en pixels.
    async fn screen_size(&mut self) -> Result<(f64, f64), String> {
        if let Some(size) = self.size {
            return Ok(size);
        }
        let sid = self.session_id().await?;
        let v: Value = self
            .client
            .get(format!("{WDA_URL}/session/{sid}/window/size"))
            .send()
            .await
            .map_err(|e| e.to_string())?
            .json()
            .await
            .map_err(|e| e.to_string())?;
        let size = (v["value"]["width"].as_f64().unwrap_or(0.0), v["value"]["height"].as_f64().unwrap_or(0.0));
        self.size = Some(size);
        Ok(size)
    }

    pub async fn tap(&mut self, nx: f64, ny: f64) -> Result<(), String> {
        let (w, h) = self.screen_size().await?;
        let x = (nx.clamp(0.0, 1.0) * w).round() as i64;
        let y = (ny.clamp(0.0, 1.0) * h).round() as i64;
        self.post(&format!("/wda/tap/{x}/{y}"), None).await
    }

    pub async fn swipe(&mut self, fx: f64, fy: f64, tx: f64, ty: f64) -> Result<(), String> {
        let (w, h) = self.screen_size().await?;
        let body = json!({
            "fromX": (fx.clamp(0.0, 1.0) * w).round() as i64,
            "fromY": (fy.clamp(0.0, 1.0) * h).round() as i64,
            "toX": (tx.clamp(0.0, 1.0) * w).round() as i64,
            "toY": (ty.clamp(0.0, 1.0) * h).round() as i64,
            "duration": 0.3,
        });
        self.post("/wda/dragfromtoforduration", Some(body)).await
    }

    pub async fn home(&mut self) -> Result<(), String> {
        self.post("/wda/homescreen", None).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn traduit_une_erreur_de_certificat_apple() {
        let raw = r#"{"time":"x","level":"INFO","msg":"getting developer image"}
{"time":"x","level":"ERROR","msg":"error mounting image","err":"MountImage: failed to get signature from Apple: getSignature: failed to send request: Post \"https://gs.apple.com/TSS/controller?action=2\": tls: failed to verify certificate: x509: certificate signed by unknown authority"}"#;
        assert!(explain_image_error(raw).contains("ne fait pas confiance"));
    }

    #[test]
    fn traduit_une_erreur_reseau() {
        let raw = r#"{"level":"ERROR","msg":"error mounting image","err":"getSignature: failed to send request: dial tcp: lookup gs.apple.com: no such host"}"#;
        assert!(explain_image_error(raw).contains("injoignable"));
    }

    #[test]
    fn garde_un_texte_brut_inconnu() {
        assert_eq!(explain_image_error("boom"), "Image développeur indisponible : boom");
    }
}
