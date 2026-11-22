import type { PageChannel } from '@lingxia/types/page';
import type { PageShape } from '../../shared/lib/page-shape';

// What the ticker channel carries each way; the View and Logic both import it.
export interface TickerUpdate {
  type: 'tick';
  symbol: string;
  price: number;
  change: number;
  timestamp: number;
}

export interface TickerInit {
  type: 'init';
  symbols: string[];
  active: string;
}

export type ServerMessage = TickerInit | TickerUpdate;

export interface ClientCommand {
  type: 'subscribe';
  symbol: string;
}

export type TickerPage = PageShape<
  Record<string, never>,
  { tickerSession(params: undefined, ch: PageChannel<ServerMessage, ClientCommand>): void }
>;
