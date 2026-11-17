import {
  useCallback,
  useEffect,
  useRef,
  useState,
  type CompositionEvent,
  type FormEvent,
  type KeyboardEvent as ReactKeyboardEvent,
  type PointerEvent as ReactPointerEvent,
  type WheelEvent as ReactWheelEvent,
} from 'react';
import type { AndroidEvent, DeviceInfo, IosEvent, IosStatus } from '../../shared/types';
import { VideoPlayer } from './lib/videoPlayer';
import { AudioPlayer } from './lib/audioPlayer';
import { IosFrameRenderer } from './lib/iosFrames';
import { ScreenRecorder, canvasToPng } from './lib/recorder';
import { keycodeFor, metaFor } from './lib/keys';
import { DeviceSwitcher, EmptyState, FilesSheet, Toasts, Toolbar, type Toast } from './components/panels';
import { IconRefresh } from './components/icons';
import { WindowControls } from './components/WindowControls';
import { UpdateBanner } from './components/UpdateBanner';

type Phase = 'idle' | 'connecting' | 'live' | 'error';

/** Réglages de la session miroir : qualité élevée, 60 images/s, décodage matériel côté PC. */
const SESSION = { maxSize: 1920, bitRate: 12_000_000, maxFps: 60 };
const BEZEL = 12;

/** État du son affiché dans la barre d'état. */
type AudioStatus = 'waiting' | 'active' | 'unavailable' | null;
const AUDIO_LABEL: Record<Exclude<AudioStatus, null>, string> = {
  waiting: 'Son : cliquez sur l’écran pour l’activer',
  active: 'Son actif',
  unavailable: 'Son indisponible sur cet appareil',
};
const MARGIN = 40;

const keyOf = (d: DeviceInfo | null) => (d ? `${d.kind}:${d.id}` : '');
const messageOf = (e: unknown) => (e instanceof Error ? e.message : String(e));
const clamp01 = (v: number) => Math.min(1, Math.max(0, v));
const fileStamp = () => new Date().toISOString().slice(0, 19).replace(/[:T]/g, '-');
const formatTime = (s: number) => `${Math.floor(s / 60)}:${String(s % 60).padStart(2, '0')}`;

/** Plus grand rectangle de ratio donné qui tient dans la zone disponible. */
function fit(width: number, height: number, aspect: number): { w: number; h: number } {
  const w = Math.max(0, Math.min(width, height * aspect));
  return { w: Math.floor(w), h: Math.floor(w / aspect) };
}

interface PointerState {
  id: number;
  sx: number;
  sy: number;
  x: number;
  y: number;
}

export function App() {
  const [devices, setDevices] = useState<DeviceInfo[]>([]);
  const [deviceErrors, setDeviceErrors] = useState<string[]>([]);
  const [selected, setSelected] = useState<DeviceInfo | null>(null);
  const [phase, setPhase] = useState<Phase>('idle');
  const [note, setNote] = useState('');
  const [hasFrame, setHasFrame] = useState(false);
  const [frame, setFrame] = useState({ width: 16, height: 10 });
  const [stage, setStage] = useState({ width: 0, height: 0 });
  const [audioOn, setAudioOn] = useState(true);
  const [recording, setRecording] = useState(false);
  const [recordSeconds, setRecordSeconds] = useState(0);
  const [iosStatus, setIosStatus] = useState<IosStatus | null>(null);
  const [toasts, setToasts] = useState<Toast[]>([]);
  const [dragOver, setDragOver] = useState(false);
  const [filesOpen, setFilesOpen] = useState(false);

  const canvasRef = useRef<HTMLCanvasElement>(null);
  const stageRef = useRef<HTMLElement>(null);
  const textRef = useRef<HTMLTextAreaElement>(null);
  const acceptDropRef = useRef(false);
  const dropRef = useRef<(paths: string[]) => void>(() => undefined);
  const videoRef = useRef<VideoPlayer | null>(null);
  const audioRef = useRef<AudioPlayer | null>(null);
  const iosRef = useRef<IosFrameRenderer | null>(null);
  const recorderRef = useRef(new ScreenRecorder());
  const audioOnRef = useRef(true);
  const audioSeenRef = useRef(false);
  const [audioStatus, setAudioStatus] = useState<AudioStatus>(null);
  const finishRecordingRef = useRef<() => Promise<void>>(async () => undefined);
  const pointerRef = useRef<PointerState | null>(null);
  const moveFrame = useRef(0);
  const pendingMove = useRef<{ x: number; y: number } | null>(null);
  const toastSeq = useRef(0);
  const wdaWarned = useRef(false);

  const notify = useCallback((text: string, tone: Toast['tone'] = 'info') => {
    const id = ++toastSeq.current;
    setToasts((list) => [...list, { id, text, tone }]);
    window.setTimeout(() => setToasts((list) => list.filter((t) => t.id !== id)), 3600);
  }, []);

  // Lecteurs vidéo / audio / iPhone, créés une fois pour le canvas unique.
  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const onSize = (width: number, height: number) => {
      setFrame({ width, height });
      setHasFrame(true);
    };
    videoRef.current = new VideoPlayer(canvas, onSize);
    iosRef.current = new IosFrameRenderer(canvas, onSize);
    audioRef.current = new AudioPlayer((message) => notify(message, 'error'));
    return () => {
      audioRef.current?.dispose();
      audioRef.current = null;
      videoRef.current = null;
      iosRef.current = null;
    };
  }, []);

  // Le son se réveille au premier clic : les navigateurs et WebView2 suspendent l'audio sans geste.
  useEffect(() => {
    const unlock = () => {
      const player = audioRef.current;
      if (!player?.suspended) return;
      void player.unlock().then(() => {
        if (audioSeenRef.current && !player.suspended) setAudioStatus('active');
      });
    };
    window.addEventListener('pointerdown', unlock, true);
    return () => window.removeEventListener('pointerdown', unlock, true);
  }, []);

  // Événements venant du processus principal.
  useEffect(() => {
    const handleAndroid = (ev: AndroidEvent) => {
      switch (ev.type) {
        case 'ready':
          setFrame({ width: ev.width, height: ev.height });
          setPhase('live');
          setNote(ev.deviceName ? `${ev.deviceName} · ${ev.width} × ${ev.height}` : `${ev.width} × ${ev.height}`);
          if (ev.audio) {
            // Sans premier paquet audio, le son est indisponible (application protégée, capture refusée).
            setAudioStatus('waiting');
            window.setTimeout(() => {
              if (!audioSeenRef.current) setAudioStatus('unavailable');
            }, 6000);
          }
          break;
        case 'size':
          setFrame({ width: ev.width, height: ev.height });
          break;
        case 'video-config':
          videoRef.current?.configure(ev.codec, ev.description);
          break;
        case 'video-frame':
          videoRef.current?.decode(ev.key, ev.timestamp, ev.data);
          break;
        case 'audio-config':
          audioRef.current?.configure(ev.description);
          audioRef.current?.setMuted(!audioOnRef.current);
          audioSeenRef.current = true;
          setAudioStatus(audioRef.current?.suspended ? 'waiting' : 'active');
          break;
        case 'audio-frame':
          audioRef.current?.decode(ev.timestamp, ev.data);
          break;
        case 'closed':
          videoRef.current?.reset();
          audioRef.current?.reset();
          setHasFrame(false);
          setPhase('error');
          setNote(ev.reason);
          void finishRecordingRef.current();
          break;
      }
    };
    const handleIos = (ev: IosEvent) => {
      if (ev.type === 'frame' && ev.data) {
        iosRef.current?.push(ev.data);
      } else if (ev.type === 'closed') {
        iosRef.current?.reset();
        setHasFrame(false);
        setPhase('error');
        setNote(ev.message ?? 'Affichage interrompu.');
        void finishRecordingRef.current();
      }
    };
    const offAndroid = window.sharescreen.onAndroidEvent(handleAndroid);
    const offIos = window.sharescreen.onIosEvent(handleIos);
    // Glisser-déposer natif : les chemins des fichiers arrivent avec l'événement.
    const offDrag = window.sharescreen.onFileDrag((ev) => {
      if (ev.type === 'drop') {
        setDragOver(false);
        dropRef.current(ev.paths);
      } else if (ev.type === 'leave') {
        setDragOver(false);
      } else {
        setDragOver(acceptDropRef.current);
      }
    });
    return () => {
      offAndroid();
      offIos();
      offDrag();
    };
  }, []);

  // Taille de la scène, pour dimensionner l'écran de l'appareil.
  useEffect(() => {
    const el = stageRef.current;
    if (!el) return;
    const ro = new ResizeObserver(([entry]) => {
      setStage({ width: entry.contentRect.width, height: entry.contentRect.height });
    });
    ro.observe(el);
    return () => ro.disconnect();
  }, []);

  // Liste des appareils, rafraîchie toutes les 5 secondes.
  const refresh = useCallback(async () => {
    try {
      const result = await window.sharescreen.listDevices();
      setDevices(result.devices);
      setDeviceErrors(result.errors);
    } catch (err) {
      setDeviceErrors([messageOf(err)]);
    }
  }, []);

  useEffect(() => {
    void refresh();
    const timer = window.setInterval(() => void refresh(), 5000);
    return () => window.clearInterval(timer);
  }, [refresh]);

  // État de l'iPhone (appairage, tunnel, WebDriverAgent) pendant qu'il est sélectionné.
  useEffect(() => {
    if (!selected || selected.kind !== 'ios') {
      setIosStatus(null);
      return;
    }
    let alive = true;
    const poll = () =>
      window.sharescreen
        .iosStatus(selected.id)
        .then((s) => {
          if (alive) setIosStatus(s);
        })
        .catch(() => undefined);
    void poll();
    const timer = window.setInterval(() => void poll(), 4000);
    return () => {
      alive = false;
      window.clearInterval(timer);
    };
  }, [selected]);

  useEffect(() => {
    audioOnRef.current = audioOn;
    audioRef.current?.setMuted(!audioOn);
  }, [audioOn]);

  // Durée d'enregistrement affichée dans la barre d'état.
  useEffect(() => {
    if (!recording) {
      setRecordSeconds(0);
      return;
    }
    const started = recorderRef.current.startedAt;
    const timer = window.setInterval(() => setRecordSeconds(Math.floor((Date.now() - started) / 1000)), 500);
    return () => window.clearInterval(timer);
  }, [recording]);

  const finishRecording = useCallback(async () => {
    if (!recorderRef.current.active) return;
    setRecording(false);
    try {
      const bytes = await recorderRef.current.stop();
      if (bytes.byteLength === 0) return;
      const path = await window.sharescreen.saveFile(`ShareScreen-${fileStamp()}.webm`, bytes, 'webm');
      notify(path ? 'Vidéo enregistrée.' : 'Enregistrement non sauvegardé.', path ? 'ok' : 'info');
    } catch (err) {
      notify(messageOf(err), 'error');
    }
  }, [notify]);

  useEffect(() => {
    finishRecordingRef.current = finishRecording;
  }, [finishRecording]);

  const stopAll = useCallback(async () => {
    await finishRecording();
    await Promise.allSettled([window.sharescreen.stopAndroid(), window.sharescreen.iosStop()]);
    videoRef.current?.reset();
    audioRef.current?.reset();
    iosRef.current?.reset();
    setSelected(null);
    setPhase('idle');
    setHasFrame(false);
    setNote('');
    setFilesOpen(false);
  }, [finishRecording]);

  const connect = useCallback(
    async (device: DeviceInfo) => {
      setSelected(device);
      setHasFrame(false);
      audioSeenRef.current = false;
      setAudioStatus(null);
      if (device.state !== 'ready') {
        setPhase('error');
        setNote(device.detail ?? 'Appareil indisponible.');
        return;
      }
      setPhase('connecting');
      setNote(device.kind === 'ios' ? 'Préparation de l\'iPhone (image développeur)…' : 'Connexion…');
      videoRef.current?.reset();
      audioRef.current?.reset();
      iosRef.current?.reset();
      await finishRecording();
      await Promise.allSettled([window.sharescreen.stopAndroid(), window.sharescreen.iosStop()]);
      try {
        if (device.kind === 'android') {
          await window.sharescreen.startAndroid({ serial: device.id, ...SESSION, audio: true, stayAwake: true });
        } else {
          await window.sharescreen.iosStart(device.id);
          setPhase('live');
          setNote('Chargement de l\'image…');
        }
      } catch (err) {
        setPhase('error');
        setNote(messageOf(err));
      }
    },
    [finishRecording],
  );

  // Coordonnées normalisées (0..1) à l'intérieur de l'image affichée.
  const toNormalized = (clientX: number, clientY: number) => {
    const canvas = canvasRef.current;
    if (!canvas) return null;
    const r = canvas.getBoundingClientRect();
    if (r.width === 0 || r.height === 0) return null;
    return { x: clamp01((clientX - r.left) / r.width), y: clamp01((clientY - r.top) / r.height) };
  };

  const isAndroidLive = selected?.kind === 'android' && phase === 'live' && hasFrame;
  const iosControl = selected?.kind === 'ios' && phase === 'live' && Boolean(iosStatus?.wda);

  const sendKey = (keycode: number, meta = 0) => window.sharescreen.sendInput({ type: 'key', keycode, meta });

  const flushMove = () => {
    moveFrame.current = 0;
    const p = pendingMove.current;
    pendingMove.current = null;
    if (p) window.sharescreen.sendInput({ type: 'touch', action: 'move', x: p.x, y: p.y });
  };

  const onPointerDown = (e: ReactPointerEvent<HTMLDivElement>) => {
    if (phase !== 'live' || !selected) return;
    const p = toNormalized(e.clientX, e.clientY);
    if (!p) return;
    if (selected.kind === 'android') {
      if (e.button === 2) return sendKey(4); // clic droit = retour
      if (e.button === 1) return sendKey(3); // clic molette = accueil
    }
    if (e.button !== 0) return;
    if (selected.kind === 'ios' && !iosControl) {
      if (!wdaWarned.current) {
        wdaWarned.current = true;
        notify('Contrôle indisponible : installez WebDriverAgent sur l\'iPhone. L\'affichage reste actif.', 'info');
      }
      return;
    }
    e.currentTarget.setPointerCapture(e.pointerId);
    pointerRef.current = { id: e.pointerId, sx: p.x, sy: p.y, x: p.x, y: p.y };
    if (selected.kind === 'android') {
      window.sharescreen.sendInput({ type: 'touch', action: 'down', x: p.x, y: p.y });
      textRef.current?.focus({ preventScroll: true });
    }
  };

  const onPointerMove = (e: ReactPointerEvent<HTMLDivElement>) => {
    const ptr = pointerRef.current;
    if (!ptr || ptr.id !== e.pointerId) return;
    const p = toNormalized(e.clientX, e.clientY);
    if (!p) return;
    ptr.x = p.x;
    ptr.y = p.y;
    if (selected?.kind === 'android') {
      pendingMove.current = p;
      if (!moveFrame.current) moveFrame.current = requestAnimationFrame(flushMove);
    }
  };

  const onPointerUp = (e: ReactPointerEvent<HTMLDivElement>) => {
    const ptr = pointerRef.current;
    if (!ptr || ptr.id !== e.pointerId) return;
    pointerRef.current = null;
    if (e.currentTarget.hasPointerCapture(e.pointerId)) e.currentTarget.releasePointerCapture(e.pointerId);
    if (selected?.kind === 'android') {
      if (moveFrame.current) {
        cancelAnimationFrame(moveFrame.current);
        moveFrame.current = 0;
        pendingMove.current = null;
      }
      window.sharescreen.sendInput({ type: 'touch', action: 'up', x: ptr.x, y: ptr.y });
      return;
    }
    const distance = Math.hypot(ptr.x - ptr.sx, ptr.y - ptr.sy);
    const action =
      distance < 0.012
        ? window.sharescreen.iosTap(ptr.sx, ptr.sy)
        : window.sharescreen.iosSwipe(ptr.sx, ptr.sy, ptr.x, ptr.y);
    action.catch((err: unknown) => notify(messageOf(err), 'error'));
  };

  const onWheel = (e: ReactWheelEvent<HTMLDivElement>) => {
    if (!isAndroidLive) return;
    const p = toNormalized(e.clientX, e.clientY);
    if (!p) return;
    const v = Math.max(-5, Math.min(5, -e.deltaY / 120));
    const h = Math.max(-5, Math.min(5, e.deltaX / 120));
    window.sharescreen.sendInput({ type: 'scroll', x: p.x, y: p.y, h, v });
  };

  const onTextKeyDown = (e: ReactKeyboardEvent<HTMLTextAreaElement>) => {
    if (!isAndroidLive) return;
    const code = keycodeFor(e.key);
    if (code !== undefined) {
      e.preventDefault();
      sendKey(code, metaFor(e));
    }
  };

  const sendText = (text: string) => {
    if (text) window.sharescreen.sendInput({ type: 'text', text });
  };

  const onTextInput = (e: FormEvent<HTMLTextAreaElement>) => {
    const el = e.currentTarget;
    if ((e.nativeEvent as InputEvent).isComposing) return;
    if (isAndroidLive) sendText(el.value);
    el.value = '';
  };

  const onTextComposed = (e: CompositionEvent<HTMLTextAreaElement>) => {
    if (isAndroidLive) sendText(e.data);
    e.currentTarget.value = '';
  };

  const pushFiles = async (paths: string[]) => {
    if (!selected || selected.kind !== 'android' || paths.length === 0) return;
    notify(`Envoi de ${paths.length} fichier${paths.length > 1 ? 's' : ''}…`);
    try {
      const results = await window.sharescreen.pushFiles(selected.id, paths);
      const failed = results.filter((r) => !r.ok);
      if (failed.length === 0) notify('Fichiers envoyés dans Téléchargements.', 'ok');
      else notify(`${failed.length} envoi${failed.length > 1 ? 's' : ''} en échec : ${failed[0].error ?? ''}`, 'error');
    } catch (err) {
      notify(messageOf(err), 'error');
    }
  };

  // Le glisser-déposer natif de Tauri (dans l'effet d'événements) lit ces deux refs à jour.
  useEffect(() => {
    acceptDropRef.current = isAndroidLive;
    dropRef.current = (paths: string[]) => void pushFiles(paths);
  });

  const pickAndPush = async () => {
    if (!selected || selected.kind !== 'android') return;
    try {
      await pushFiles(await window.sharescreen.pickFiles());
    } catch (err) {
      notify(messageOf(err), 'error');
    }
  };

  const screenshot = async () => {
    const canvas = canvasRef.current;
    if (!canvas || !hasFrame) return;
    try {
      const bytes = await canvasToPng(canvas);
      const path = await window.sharescreen.saveFile(`ShareScreen-${fileStamp()}.png`, bytes, 'png');
      if (path) notify('Capture enregistrée.', 'ok');
    } catch (err) {
      notify(messageOf(err), 'error');
    }
  };

  const toggleRecording = async () => {
    const canvas = canvasRef.current;
    if (!canvas || !hasFrame) return;
    if (recorderRef.current.active) {
      await finishRecording();
      return;
    }
    try {
      const audioTrack = selected?.kind === 'android' ? (audioRef.current?.audioTrack ?? null) : null;
      recorderRef.current.start(canvas, audioTrack, selected?.kind === 'ios' ? 30 : 60);
      setRecording(true);
    } catch (err) {
      notify(messageOf(err), 'error');
    }
  };

  const tunnel = async () => {
    try {
      await window.sharescreen.iosTunnel();
      notify('Demande de droits administrateur envoyée. Validez la fenêtre Windows.', 'info');
      window.setTimeout(() => void window.sharescreen.iosStatus(selected?.id ?? null).then(setIosStatus), 4000);
    } catch (err) {
      notify(messageOf(err), 'error');
    }
  };

  const homeIos = () => window.sharescreen.iosHome().catch((err: unknown) => notify(messageOf(err), 'error'));

  const pairIos = async () => {
    if (!selected) return;
    notify('Demande d\'appairage envoyée. Acceptez « Faire confiance » sur l\'iPhone.', 'info');
    try {
      await window.sharescreen.iosPair(selected.id);
      notify('Appairage réussi.', 'ok');
    } catch (err) {
      notify(messageOf(err), 'error');
    }
  };

  const revealIos = async () => {
    if (!selected) return;
    try {
      await window.sharescreen.iosReveal(selected.id);
      notify('Option révélée : Réglages > Confidentialité et sécurité > Mode développeur.', 'info');
    } catch (err) {
      notify(messageOf(err), 'error');
    }
  };

  // Mise en page : écran de l'appareil au ratio de l'image, dans la scène disponible.
  const aspect = frame.width / frame.height;
  const box = fit(Math.max(0, stage.width - MARGIN * 2 - BEZEL * 2), Math.max(0, stage.height - MARGIN * 2 - BEZEL * 2), aspect);
  const portrait = frame.height > frame.width;
  const showFrame = phase === 'live' && hasFrame;

  const liveLabel = !hasFrame
    ? 'Chargement de l\'image…'
    : selected?.kind === 'ios'
      ? `${selected.name} · ${frame.width} × ${frame.height}${iosStatus && !iosStatus.wda ? ' · affichage seul' : ''}`
      : note;

  const statusText =
    phase === 'live'
      ? liveLabel
      : phase === 'connecting'
        ? 'Connexion…'
        : phase === 'error'
          ? note
          : devices.length > 0
            ? 'Choisissez un appareil pour commencer'
            : (deviceErrors[0] ?? 'Branchez une tablette ou un iPhone');

  return (
    <div className="app">
      <header className="topbar" data-tauri-drag-region>
        <div className="brand" data-tauri-drag-region>
          <span className="logo" aria-hidden="true" />
          <span>ShareScreen</span>
        </div>
        <DeviceSwitcher
          devices={devices}
          activeKey={keyOf(selected)}
          connecting={phase === 'connecting'}
          onPick={(d) => {
            if (keyOf(d) === keyOf(selected) && phase === 'live') return;
            void connect(d);
          }}
        />
        <div className="topbar-end">
          <button className="icon-btn" title="Actualiser la liste" aria-label="Actualiser la liste" onClick={() => void refresh()}>
            <IconRefresh />
          </button>
          <WindowControls />
        </div>
      </header>

      <UpdateBanner />

      <main className="workspace">
        <section className={`stage${dragOver ? ' is-drop' : ''}`} ref={stageRef}>
          <div
            className={`device${showFrame ? '' : ' is-hidden'}`}
            data-kind={selected?.kind ?? 'android'}
            data-orient={portrait ? 'portrait' : 'landscape'}
            style={{ width: box.w + BEZEL * 2, height: box.h + BEZEL * 2 }}
          >
            <div
              className="screen"
              style={{ width: box.w, height: box.h }}
              onPointerDown={onPointerDown}
              onPointerMove={onPointerMove}
              onPointerUp={onPointerUp}
              onPointerCancel={onPointerUp}
              onWheel={onWheel}
              onContextMenu={(e) => e.preventDefault()}
            >
              <canvas ref={canvasRef} className="mirror" />
            </div>
          </div>
          {!showFrame && (
            <EmptyState
              selected={selected}
              phase={phase === 'live' ? 'connecting' : phase}
              note={note}
              iosStatus={iosStatus}
              onPair={() => void pairIos()}
              onReveal={() => void revealIos()}
              onTunnel={() => void tunnel()}
              onRetry={() => (selected ? void connect(selected) : undefined)}
            />
          )}
          {dragOver && (
            <div className="drop-overlay">
              <div>Déposez les fichiers pour les envoyer vers Téléchargements</div>
            </div>
          )}
        </section>

        {selected && (
          <Toolbar
            kind={selected.kind}
            live={showFrame}
            canInput={selected.kind === 'android' ? isAndroidLive : iosControl}
            audioOn={audioOn}
            recording={recording}
            onKey={(keycode) => sendKey(keycode)}
            onHome={() => void homeIos()}
            onRotate={() => window.sharescreen.sendInput({ type: 'rotate' })}
            onAudio={() => setAudioOn((v) => !v)}
            onScreenshot={() => void screenshot()}
            onRecord={() => void toggleRecording()}
            onFiles={() => setFilesOpen(true)}
            onStop={() => void stopAll()}
          />
        )}
      </main>

      <footer className="statusbar">
        <span className={`dot ${phase}`} aria-hidden="true" />
        <span className="status-text">{statusText}</span>
        <span className="spacer" />
        {audioStatus && selected?.kind === 'android' && (
          <span className={`audio-state is-${audioStatus}`}>{AUDIO_LABEL[audioStatus]}</span>
        )}
        {recording && <span className="rec" aria-label="Enregistrement en cours">● REC {formatTime(recordSeconds)}</span>}
      </footer>

      <textarea
        ref={textRef}
        className="ime"
        aria-hidden="true"
        tabIndex={-1}
        autoCapitalize="off"
        autoCorrect="off"
        spellCheck={false}
        onKeyDown={onTextKeyDown}
        onInput={onTextInput}
        onCompositionEnd={onTextComposed}
      />
      <Toasts toasts={toasts} />
      {filesOpen && selected?.kind === 'android' && (
        <FilesSheet
          serial={selected.id}
          onClose={() => setFilesOpen(false)}
          onPickFiles={() => {
            setFilesOpen(false);
            void pickAndPush();
          }}
          onNotify={notify}
        />
      )}
    </div>
  );
}
