// Types partagés entre le processus principal, le preload et l'interface.

export type DeviceKind = 'android' | 'ios';

export interface DeviceInfo {
  kind: DeviceKind;
  id: string; // numéro de série adb ou UDID iOS
  name: string;
  state: 'ready' | 'unauthorized' | 'offline' | 'untrusted' | 'unavailable';
  detail?: string; // version et état lisibles
  hint?: string; // conseil propre à la marque ou au système
}

export interface AndroidOptions {
  serial: string;
  maxSize: number; // plus grand côté de l'image encodée
  bitRate: number; // bits par seconde
  maxFps: number;
  audio: boolean;
  stayAwake: boolean;
}

export type AndroidEvent =
  | { type: 'ready'; width: number; height: number; deviceName: string; audio: boolean }
  | { type: 'size'; width: number; height: number }
  | { type: 'video-config'; codec: string; description: Uint8Array }
  | { type: 'video-frame'; key: boolean; timestamp: number; data: Uint8Array }
  | { type: 'audio-config'; description: Uint8Array }
  | { type: 'audio-frame'; timestamp: number; data: Uint8Array }
  | { type: 'closed'; reason: string };

export type InputEvent =
  | { type: 'touch'; action: 'down' | 'move' | 'up'; x: number; y: number }
  | { type: 'scroll'; x: number; y: number; h: number; v: number }
  | { type: 'key'; keycode: number; meta?: number }
  | { type: 'text'; text: string }
  | { type: 'back' }
  | { type: 'power'; on: boolean }
  | { type: 'rotate' };

export interface IosEvent {
  type: 'frame' | 'closed' | 'error';
  data?: Uint8Array; // JPEG pour 'frame'
  message?: string;
}

export interface IosStatus {
  ios: boolean; // ios.exe présent
  version: string; // ex. "18.2", vide si inconnue
  paired: boolean;
  developerMode: boolean | null; // null si inconnu ou non applicable (iOS < 16)
  tunnelRequired: boolean; // iOS 17 et plus
  tunnel: boolean; // tunnel actif, ou inutile
  wda: boolean; // WebDriverAgent joignable (requis pour le contrôle)
  message: string;
}
