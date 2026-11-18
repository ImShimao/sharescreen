/**
 * Son de la tablette : décodage Opus (WebCodecs) puis lecture planifiée sur l'AudioContext.
 * Un petit tampon (≈60 ms) absorbe la gigue réseau sans faire grossir la latence.
 */
const SAMPLE_RATE = 48000;
const BUFFER_SECONDS = 0.06;

export class AudioPlayer {
  private context: AudioContext | null = null;
  private failed = false;

  constructor(private readonly onError: (message: string) => void = () => undefined) {}

  /** Vrai tant que le navigateur garde l'audio suspendu (il faut un clic sur la page). */
  get suspended(): boolean {
    return this.context?.state === 'suspended';
  }

  /** Réveille l'audio au premier geste de l'utilisateur. */
  async unlock(): Promise<void> {
    if (this.context?.state === 'suspended') await this.context.resume();
  }

  private fail(err: unknown): void {
    console.error('[audio]', err);
    if (!this.failed) {
      this.failed = true;
      this.onError(`Son : décodage impossible (${err instanceof Error ? err.message : String(err)}).`);
    }
  }
  private decoder: AudioDecoder | null = null;
  private gain: GainNode | null = null;
  private destination: MediaStreamAudioDestinationNode | null = null;
  private nextStart = 0;
  private muted = false;

  private ensureContext(): AudioContext {
    if (!this.context) {
      this.context = new AudioContext({ sampleRate: SAMPLE_RATE, latencyHint: 'interactive' });
      this.gain = this.context.createGain();
      this.gain.gain.value = this.muted ? 0 : 1;
      this.destination = this.context.createMediaStreamDestination();
      this.gain.connect(this.context.destination);
      this.gain.connect(this.destination);
    }
    void this.context.resume();
    return this.context;
  }

  /** Piste audio du son de la tablette, pour l'enregistrement. */
  get audioTrack(): MediaStreamTrack | null {
    return this.destination?.stream.getAudioTracks()[0] ?? null;
  }

  configure(description: Uint8Array): void {
    this.ensureContext();
    this.decoder?.close();
    this.decoder = new AudioDecoder({
      output: (data) => this.play(data),
      error: (err) => this.fail(err),
    });
    this.decoder.configure({ codec: 'opus', numberOfChannels: 2, sampleRate: SAMPLE_RATE, description });
    this.failed = false;
    this.nextStart = 0;
  }

  decode(timestamp: number, data: Uint8Array): void {
    if (!this.decoder || this.decoder.state !== 'configured') return;
    this.decoder.decode(new EncodedAudioChunk({ type: 'key', timestamp, data }));
  }

  setMuted(muted: boolean): void {
    this.muted = muted;
    if (this.gain) this.gain.gain.value = muted ? 0 : 1;
  }

  reset(): void {
    this.decoder?.close();
    this.decoder = null;
    this.nextStart = 0;
  }

  private play(data: AudioData): void {
    const ctx = this.context;
    if (!ctx || !this.gain) {
      data.close();
      return;
    }
    const channels = Math.min(data.numberOfChannels, 2);
    const buffer = ctx.createBuffer(channels, data.numberOfFrames, data.sampleRate);
    for (let ch = 0; ch < channels; ch++) {
      const samples = new Float32Array(data.numberOfFrames);
      data.copyTo(samples, { planeIndex: ch, format: 'f32-planar' });
      buffer.copyToChannel(samples, ch);
    }
    data.close();

    const source = ctx.createBufferSource();
    source.buffer = buffer;
    source.connect(this.gain);
    const now = ctx.currentTime;
    // Recale la file si elle a pris du retard ou s'est trop éloignée.
    if (this.nextStart < now + 0.01 || this.nextStart > now + 0.25) this.nextStart = now + BUFFER_SECONDS;
    source.start(this.nextStart);
    this.nextStart += buffer.duration;
  }

  dispose(): void {
    this.reset();
    void this.context?.close();
    this.context = null;
    this.gain = null;
    this.destination = null;
  }
}
