/**
 * Dessine les images JPEG de l'iPhone sur le canvas. Une seule décodage à la fois :
 * si une image arrive pendant le décodage, seule la plus récente est gardée.
 */
export class IosFrameRenderer {
  private busy = false;
  private latest: Uint8Array | null = null;
  private readonly ctx: CanvasRenderingContext2D | null;

  constructor(
    private readonly canvas: HTMLCanvasElement,
    private readonly onSize: (width: number, height: number) => void,
  ) {
    this.ctx = canvas.getContext('2d', { alpha: false, desynchronized: true });
  }

  push(jpeg: Uint8Array): void {
    this.latest = jpeg;
    if (!this.busy) void this.drain();
  }

  private async drain(): Promise<void> {
    this.busy = true;
    while (this.latest) {
      const jpeg = this.latest;
      this.latest = null;
      const bytes = new Uint8Array(jpeg.byteLength);
      bytes.set(jpeg);
      const bitmap = await createImageBitmap(new Blob([bytes], { type: 'image/jpeg' })).catch(() => null);
      if (!bitmap) continue;
      if (this.canvas.width !== bitmap.width || this.canvas.height !== bitmap.height) {
        this.canvas.width = bitmap.width;
        this.canvas.height = bitmap.height;
        this.onSize(bitmap.width, bitmap.height);
      }
      this.ctx?.drawImage(bitmap, 0, 0);
      bitmap.close();
    }
    this.busy = false;
  }

  reset(): void {
    this.latest = null;
    this.ctx?.clearRect(0, 0, this.canvas.width, this.canvas.height);
  }
}
