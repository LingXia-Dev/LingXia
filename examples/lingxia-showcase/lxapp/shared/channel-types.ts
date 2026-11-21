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
