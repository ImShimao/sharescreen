/**
 * Enregistrement de ce qui est affiché (image + son de la tablette) en WebM, sans dépendance externe.
 */
export class ScreenRecorder {
  private recorder: MediaRecorder | null = null;
  private chunks: Blob[] = [];
  startedAt = 0;

  start(canvas: HTMLCanvasElement, audioTrack: MediaStreamTrack | null, fps: number): void {
    const tracks: MediaStreamTrack[] = [...canvas.captureStream(fps).getVideoTracks()];
    if (audioTrack) tracks.push(audioTrack);
    const stream = new MediaStream(tracks);
    const mimeType = MediaRecorder.isTypeSupported('video/webm;codecs=vp9,opus')
      ? 'video/webm;codecs=vp9,opus'
      : 'video/webm';
    this.chunks = [];
    this.recorder = new MediaRecorder(stream, { mimeType, videoBitsPerSecond: 10_000_000, audioBitsPerSecond: 128_000 });
    this.recorder.ondataavailable = (e) => {
      if (e.data.size > 0) this.chunks.push(e.data);
    };
    this.recorder.start(1000);
    this.startedAt = Date.now();
  }

  get active(): boolean {
    return this.recorder?.state === 'recording';
  }

  stop(): Promise<Uint8Array> {
    const recorder = this.recorder;
    if (!recorder) return Promise.resolve(new Uint8Array());
    return new Promise((resolve) => {
      recorder.onstop = async () => {
        const blob = new Blob(this.chunks, { type: 'video/webm' });
        this.recorder = null;
        this.chunks = [];
        resolve(new Uint8Array(await blob.arrayBuffer()));
      };
      recorder.stop();
    });
  }
}

/** Capture PNG de l'image affichée. */
export function canvasToPng(canvas: HTMLCanvasElement): Promise<Uint8Array> {
  return new Promise((resolve, reject) => {
    canvas.toBlob(async (blob) => {
      if (!blob) {
        reject(new Error('Capture impossible.'));
        return;
      }
      resolve(new Uint8Array(await blob.arrayBuffer()));
    }, 'image/png');
  });
}
