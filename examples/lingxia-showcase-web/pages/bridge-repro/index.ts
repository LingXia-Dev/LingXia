// Bridge delivery check: Logic emits a gap-free tick sequence and the View
// audits what arrives. A missing seq means the transport dropped a frame.
import { definePage } from '@lingxia/types/page';
import type { BootstrapProbe, BridgeReproPage, Tick } from './contract';

const TICK_INTERVAL_MS = 50;
const TICK_LIMIT = 200000;
const LOGIC_LOADED_AT = Date.now();

const initialData: BridgeReproPage['data'] = {
  bootstrapMarker: 'initial logic snapshot received',
  logicLoadedAt: LOGIC_LOADED_AT,
};

export default definePage({
  data: initialData,
  setup: page => ({
    actions: {
      // The View calls it as it mounts, so a stalled bootstrap shows as a late answer.
      onBootstrapProbe({ nonce, viewSentAt }: { nonce: number; viewSentAt: number }): BootstrapProbe {
        return { nonce, viewSentAt, logicReceivedAt: Date.now(), bootstrapMarker: page.data.bootstrapMarker };
      },
      async *onTicks(): AsyncGenerator<Tick, void> {
        for (let seq = 1; seq <= TICK_LIMIT; seq++) {
          await new Promise<void>((r) => setTimeout(r, TICK_INTERVAL_MS));
          yield { seq, ts: Date.now() };
        }
      },
      // A unary round trip, to check the bridge answers while idle.
      onEcho({ n }: { n: number }): { n: number; ts: number } {
        return { n, ts: Date.now() };
      },
    },
  }),
});
