// ChannelHandle is injected by the runtime — no import needed.
// The type is declared here locally for readability.
interface ChannelHandle<TSend = unknown, TReceive = unknown> {
  send(payload: TSend): void;
  close(code?: string, reason?: string): void;
  on(event: 'data', handler: (payload: TReceive) => void): void;
  on(event: 'close', handler: (info: { code: string; reason: string }) => void): void;
}

// ---------------------------------------------------------------------------
// Channel demo — a mock real-time stock ticker.
//
// The View opens a channel; Logic pushes price updates at a fixed interval.
// The View can send commands ("subscribe") to change the watched symbol.
// This demonstrates bidirectional channel communication using ch.on().
// ---------------------------------------------------------------------------

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

const SYMBOLS: Record<string, { base: number; volatility: number }> = {
  'AAPL':  { base: 189.5,  volatility: 2.5 },
  'GOOGL': { base: 141.2,  volatility: 3.1 },
  'MSFT':  { base: 378.9,  volatility: 4.0 },
  'TSLA':  { base: 248.6,  volatility: 8.5 },
};

function randomPrice(base: number, volatility: number): number {
  const delta = (Math.random() - 0.5) * 2 * volatility;
  return Math.round((base + delta) * 100) / 100;
}

Page({
  data: {
    connected: false,
  },

  tickerSession(
    _params: Record<string, unknown>,
    ch: ChannelHandle<ServerMessage, ClientCommand>,
  ) {
    let activeSymbol = 'AAPL';
    let timer: ReturnType<typeof setInterval> | null = null;
    let lastPrice = SYMBOLS[activeSymbol].base;

    const startTicking = () => {
      if (timer) clearInterval(timer);
      lastPrice = SYMBOLS[activeSymbol].base;

      timer = setInterval(() => {
        const cfg = SYMBOLS[activeSymbol];
        if (!cfg) return;
        const price = randomPrice(cfg.base, cfg.volatility);
        const change = Math.round((price - lastPrice) * 100) / 100;
        lastPrice = price;
        ch.send({
          type: 'tick',
          symbol: activeSymbol,
          price,
          change,
          timestamp: Date.now(),
        });
      }, 800);
    };

    // Send initial state
    ch.send({
      type: 'init',
      symbols: Object.keys(SYMBOLS),
      active: activeSymbol,
    });

    startTicking();

    this.setData({ connected: true });

    // Register listeners for incoming data and close events.
    ch.on('data', (msg: ClientCommand) => {
      if (msg.type === 'subscribe' && SYMBOLS[msg.symbol]) {
        activeSymbol = msg.symbol;
        startTicking();
      }
    });

    ch.on('close', () => {
      if (timer) clearInterval(timer);
      timer = null;
    });
  },
});
