import type { AndroidEvent, AndroidOptions, DeviceInfo, InputEvent, IosEvent, IosStatus } from './types';

/** Glisser-déposer de fichiers sur la fenêtre (fournis par Tauri). */
export type FileDragEvent = { type: 'enter' | 'over' | 'leave' } | { type: 'drop'; paths: string[] };

/** Surface exposée à l'interface (window.sharescreen), implémentée par lib/backend.ts. */
export interface ShareScreenApi {
  listDevices(): Promise<{ devices: DeviceInfo[]; errors: string[] }>;
  startAndroid(options: AndroidOptions): Promise<void>;
  stopAndroid(): Promise<void>;
  sendInput(event: InputEvent): void;
  onAndroidEvent(listener: (event: AndroidEvent) => void): () => void;
  listFiles(serial: string): Promise<string[]>;
  pickFiles(): Promise<string[]>;
  pushFiles(serial: string, localPaths: string[]): Promise<{ name: string; ok: boolean; error?: string }[]>;
  pullFile(serial: string, name: string): Promise<string | null>;
  onFileDrag(listener: (event: FileDragEvent) => void): () => void;
  iosStatus(udid: string | null): Promise<IosStatus>;
  iosStart(udid: string): Promise<void>;
  iosStop(): Promise<void>;
  onIosEvent(listener: (event: IosEvent) => void): () => void;
  iosTunnel(): Promise<void>;
  iosPair(udid: string): Promise<void>;
  iosReveal(udid: string): Promise<void>;
  iosTap(nx: number, ny: number): Promise<void>;
  iosSwipe(fx: number, fy: number, tx: number, ty: number): Promise<void>;
  iosHome(): Promise<void>;
  appleCertsInstall(): Promise<void>;
  saveFile(suggestedName: string, bytes: Uint8Array, extension: string): Promise<string | null>;
}

declare global {
  interface Window {
    sharescreen: ShareScreenApi;
  }
}
