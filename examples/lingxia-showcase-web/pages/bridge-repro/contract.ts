import type { LxStream } from '@lingxia/bridge';
import type { PageContract } from '@lingxia/types/page';

export interface Tick {
  seq: number;
  ts: number;
}

export interface BootstrapProbe {
  nonce: number;
  viewSentAt: number;
  logicReceivedAt: number;
  bootstrapMarker: string;
}

export type BridgeReproPage = PageContract<
  { bootstrapMarker: string; logicLoadedAt: number },
  {
    onBootstrapProbe(params: { nonce: number; viewSentAt: number }): Promise<BootstrapProbe>;
    onTicks(): LxStream<Tick, void>;
    onEcho(params: { n: number }): Promise<{ n: number; ts: number }>;
  }
>;
