/**
 * Implémentation de window.sharescreen au-dessus de Tauri : commandes (invoke), canaux binaires
 * pour le flux vidéo, plugin de dialogue pour les fichiers, glisser-déposer natif.
 */
import { Channel, invoke } from '@tauri-apps/api/core';
import { getCurrentWebview } from '@tauri-apps/api/webview';
import { open, save } from '@tauri-apps/plugin-dialog';
import type { AndroidEvent, AndroidOptions, DeviceInfo, InputEvent, IosEvent, IosStatus } from '../../../shared/types';
import type { FileDragEvent, ShareScreenApi } from '../../../shared/api';

// Types de messages binaires : voir src-tauri/src/events.rs.
const READY = 1;
const SIZE = 2;
const VIDEO_CONFIG = 3;
const VIDEO_FRAME = 4;
const AUDIO_CONFIG = 5;
const AUDIO_FRAME = 6;
const CLOSED = 7;
const IOS_FRAME = 8;

const text = new TextDecoder();

function decodeAndroid(buf: ArrayBuffer): AndroidEvent | null {
  const view = new DataView(buf);
  const bytes = new Uint8Array(buf);
  switch (view.getUint8(0)) {
    case READY: {
      const j = JSON.parse(text.decode(bytes.subarray(1)));
      return { type: 'ready', width: j.width, height: j.height, deviceName: j.deviceName, audio: j.audio };
    }
    case SIZE: {
      const j = JSON.parse(text.decode(bytes.subarray(1)));
      return { type: 'size', width: j.width, height: j.height };
    }
    case VIDEO_CONFIG: {
      const len = view.getUint16(1);
      const codec = text.decode(bytes.subarray(3, 3 + len));
      return { type: 'video-config', codec, description: bytes.slice(3 + len) };
    }
    case VIDEO_FRAME:
      return {
        type: 'video-frame',
        key: view.getUint8(1) === 1,
        timestamp: Number(view.getBigUint64(2)),
        data: bytes.slice(10),
      };
    case AUDIO_CONFIG:
      return { type: 'audio-config', description: bytes.slice(1) };
    case AUDIO_FRAME:
      return { type: 'audio-frame', timestamp: Number(view.getBigUint64(1)), data: bytes.slice(9) };
    case CLOSED:
      return { type: 'closed', reason: text.decode(bytes.subarray(1)) };
    default:
      return null;
  }
}

function decodeIos(buf: ArrayBuffer): IosEvent | null {
  const bytes = new Uint8Array(buf);
  switch (new DataView(buf).getUint8(0)) {
    case IOS_FRAME:
      return { type: 'frame', data: bytes.slice(1) };
    case CLOSED:
      return { type: 'closed', message: text.decode(bytes.subarray(1)) };
    default:
      return null;
  }
}

export function createBackend(): ShareScreenApi {
  const androidListeners = new Set<(e: AndroidEvent) => void>();
  const iosListeners = new Set<(e: IosEvent) => void>();
  // Chaque démarrage invalide les événements de la session précédente.
  let androidSession = 0;
  let iosSession = 0;

  return {
    listDevices: () => invoke<{ devices: DeviceInfo[]; errors: string[] }>('devices_list'),

    startAndroid: async (options: AndroidOptions) => {
      const session = ++androidSession;
      const channel = new Channel<ArrayBuffer>();
      channel.onmessage = (buf) => {
        if (session !== androidSession) return;
        const event = decodeAndroid(buf);
        if (event) androidListeners.forEach((l) => l(event));
      };
      await invoke('android_start', { options, onEvent: channel });
    },

    stopAndroid: async () => {
      androidSession++;
      await invoke('android_stop');
    },

    sendInput: (event: InputEvent) => {
      void invoke('android_input', { event });
    },

    onAndroidEvent: (listener) => {
      androidListeners.add(listener);
      return () => androidListeners.delete(listener);
    },

    listFiles: (serial: string) => invoke<string[]>('files_list', { serial }),

    pickFiles: async () => {
      const picked = await open({ multiple: true, directory: false });
      if (!picked) return [];
      return Array.isArray(picked) ? picked : [picked];
    },

    pushFiles: (serial: string, localPaths: string[]) =>
      invoke<{ name: string; ok: boolean; error?: string }[]>('files_push', { serial, paths: localPaths }),

    pullFile: async (serial: string, name: string) => {
      const path = await save({ defaultPath: name });
      if (!path) return null;
      await invoke('files_pull', { serial, name, localPath: path });
      return path;
    },

    onFileDrag: (listener: (event: FileDragEvent) => void) => {
      let unlisten: (() => void) | undefined;
      void getCurrentWebview()
        .onDragDropEvent((event) => {
          const p = event.payload;
          if (p.type === 'drop') listener({ type: 'drop', paths: p.paths });
          else if (p.type === 'leave') listener({ type: 'leave' });
          else listener({ type: p.type });
        })
        .then((fn) => {
          unlisten = fn;
        })
        .catch(() => undefined);
      return () => unlisten?.();
    },

    iosStatus: (udid: string | null) => invoke<IosStatus>('ios_status', { udid }),

    iosStart: async (udid: string) => {
      const session = ++iosSession;
      const channel = new Channel<ArrayBuffer>();
      channel.onmessage = (buf) => {
        if (session !== iosSession) return;
        const event = decodeIos(buf);
        if (event) iosListeners.forEach((l) => l(event));
      };
      await invoke('ios_start', { udid, onEvent: channel });
    },

    iosStop: async () => {
      iosSession++;
      await invoke('ios_stop');
    },

    onIosEvent: (listener) => {
      iosListeners.add(listener);
      return () => iosListeners.delete(listener);
    },

    iosTunnel: () => invoke('ios_tunnel'),
    iosPair: (udid: string) => invoke('ios_pair', { udid }),
    iosReveal: (udid: string) => invoke('ios_reveal', { udid }),
    iosTap: (nx: number, ny: number) => invoke('ios_tap', { nx, ny }),
    iosSwipe: (fx: number, fy: number, tx: number, ty: number) => invoke('ios_swipe', { fx, fy, tx, ty }),
    iosHome: () => invoke('ios_home'),

    saveFile: async (suggestedName: string, bytes: Uint8Array, extension: string) => {
      const path = await save({
        defaultPath: suggestedName,
        filters: [{ name: extension.toUpperCase(), extensions: [extension] }],
      });
      if (!path) return null;
      // Corps binaire ; le chemin voyage dans un en-tête pourcentage-encodé.
      await invoke('save_bytes', bytes, { headers: { path: encodeURIComponent(path) } });
      return path;
    },
  };
}
