// Bridge delivery repro: the logic layer emits a gap-free tick sequence; the
// view audits what actually arrives. Any missing seq means the transport
// dropped a frame (Apple downstream queue loss around connect/replace).

export interface Tick {
  seq: number;
  ts: number;
}

const TICK_INTERVAL_MS = 50;
const TICK_LIMIT = 200000;

Page({
  data: {},

  async *onTicks(): AsyncGenerator<Tick, void> {
    for (let seq = 1; seq <= TICK_LIMIT; seq++) {
      await new Promise<void>((r) => setTimeout(r, TICK_INTERVAL_MS));
      yield { seq, ts: Date.now() };
    }
  },

  // Unary echo used by the view to measure bridge round-trip health.
  onEcho(params: { n: number }): { n: number; ts: number } {
    return { n: params?.n ?? 0, ts: Date.now() };
  },
});
