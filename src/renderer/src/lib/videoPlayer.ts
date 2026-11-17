/**
 * Décodage H.264 matériel (WebCodecs) et affichage sur un canvas.
 * Seule la dernière image est dessinée à chaque rafraîchissement : pas de file d'attente qui s'allonge.
 */
export class VideoPlayer {
  private decoder: VideoDecoder | null = null;
  private pending: VideoFrame | null = null;
  private frameRequest = 0;
  private waitingForKey = true;
  private readonly ctx: CanvasRenderingContext2D | null;

  constructor(
    private readonly canvas: HTMLCanvasElement,
    private readonly onSize: (width: number, height: number) => void,
  ) {
    this.ctx = canvas.getContext('2d', { alpha: false, desynchronized: true });
  }

  configure(codec: string, description: Uint8Array): void {
    this.decoder?.close();
    this.decoder = new VideoDecoder({
      output: (frame) => this.enqueue(frame),
      error: () => {
        // Le décodeur a perdu sa référence : on attend la prochaine image clé.
        this.waitingForKey = true;
      },
    });
    this.decoder.configure({
      codec,
      description,
      optimizeForLatency: true,
      hardwareAcceleration: 'prefer-hardware',
    });
    this.waitingForKey = true;
  }

  decode(key: boolean, timestamp: number, data: Uint8Array): void {
    const decoder = this.decoder;
    if (!decoder || decoder.state !== 'configured') return;
    if (this.waitingForKey && !key) return;
    this.waitingForKey = false;
    decoder.decode(new EncodedVideoChunk({ type: key ? 'key' : 'delta', timestamp, data }));
  }

  reset(): void {
    this.decoder?.close();
    this.decoder = null;
    this.pending?.close();
    this.pending = null;
    if (this.frameRequest) cancelAnimationFrame(this.frameRequest);
    this.frameRequest = 0;
    this.ctx?.clearRect(0, 0, this.canvas.width, this.canvas.height);
  }

  private enqueue(frame: VideoFrame): void {
    this.pending?.close();
    this.pending = frame;
    if (!this.frameRequest) this.frameRequest = requestAnimationFrame(this.draw);
  }

  private readonly draw = (): void => {
    this.frameRequest = 0;
    const frame = this.pending;
    this.pending = null;
    if (!frame) return;
    const w = frame.displayWidth;
    const h = frame.displayHeight;
    if (this.canvas.width !== w || this.canvas.height !== h) {
      this.canvas.width = w;
      this.canvas.height = h;
      this.onSize(w, h);
    }
    this.ctx?.drawImage(frame, 0, 0, w, h);
    frame.close();
  };
}

/** Vérifie que WebCodecs et le décodage H.264 matériel sont disponibles. */
export async function videoSupported(codec = 'avc1.640028'): Promise<boolean> {
  if (typeof VideoDecoder === 'undefined') return false;
  const support = await VideoDecoder.isConfigSupported({ codec, hardwareAcceleration: 'prefer-hardware' }).catch(() => null);
  return Boolean(support?.supported);
}
