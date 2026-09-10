export class Drafts {
  private pending: string | undefined;
  private running: Promise<void> | undefined;
  private timer: ReturnType<typeof setTimeout> | undefined;
  private lastSent = 0;
  private stopped = false;

  constructor(
    private send: (text: string) => Promise<void>,
    private failed: (error: unknown) => void,
    private interval = 3000,
  ) {}

  request(text: string) {
    if (this.stopped) return;
    this.pending = text;
    this.schedule();
  }

  private schedule() {
    if (
      this.timer ||
      this.running ||
      this.stopped ||
      this.pending === undefined
    )
      return;
    this.timer = setTimeout(
      () => {
        this.timer = undefined;
        void this.drain(false).catch(this.failed);
      },
      Math.max(
        0,
        this.lastSent + this.interval - Date.now(),
        this.lastSent ? 0 : this.interval,
      ),
    );
  }

  private async drain(immediate: boolean): Promise<void> {
    while (this.running) await this.running;
    if (this.pending === undefined || this.stopped) return;
    const text = this.pending;
    this.pending = undefined;
    this.lastSent = Date.now();
    this.running = this.send(text);
    try {
      await this.running;
    } catch (error) {
      this.pending ??= text;
      throw error;
    } finally {
      this.running = undefined;
    }
    if (immediate) await this.drain(true);
    else this.schedule();
  }

  async flush(text?: string) {
    clearTimeout(this.timer);
    this.timer = undefined;
    if (text !== undefined) this.pending = text;
    await this.drain(true);
  }

  async stop() {
    this.stopped = true;
    clearTimeout(this.timer);
    this.pending = undefined;
    await this.running?.catch(() => {});
  }
}
