// A mock stock ticker: the View opens the channel, Logic pushes a price every
// 800 ms, and the View sends `subscribe` to change the watched symbol.
import { definePage, type PageChannel } from '@lingxia/logic';

// What the ticker channel carries each way.
interface TickerUpdate {
  type: 'tick';
  symbol: string;
  price: number;
  change: number;
  timestamp: number;
}

interface TickerInit {
  type: 'init';
  symbols: string[];
  active: string;
}

type ServerMessage = TickerInit | TickerUpdate;

interface ClientCommand {
  type: 'subscribe';
  symbol: string;
}

const SYMBOLS: Record<string, { base: number; volatility: number }> = {
  AAPL: { base: 189.5, volatility: 2.5 },
  GOOGL: { base: 141.2, volatility: 3.1 },
  MSFT: { base: 378.9, volatility: 4.0 },
  TSLA: { base: 248.6, volatility: 8.5 },
};

function randomPrice(base: number, volatility: number): number {
  const delta = (Math.random() - 0.5) * 2 * volatility;
  return Math.round((base + delta) * 100) / 100;
}

export default definePage({
  setup: () => ({
    actions: {
      tickerSession(_params: undefined, ch: PageChannel<ServerMessage, ClientCommand>) {
        let activeSymbol = 'AAPL';
        let lastPrice = SYMBOLS[activeSymbol].base;
        let timer: ReturnType<typeof setInterval> | null = null;

        const startTicking = () => {
          if (timer) clearInterval(timer);
          lastPrice = SYMBOLS[activeSymbol].base;
          timer = setInterval(() => {
            const { base, volatility } = SYMBOLS[activeSymbol];
            const price = randomPrice(base, volatility);
            const change = Math.round((price - lastPrice) * 100) / 100;
            lastPrice = price;
            ch.send({ type: 'tick', symbol: activeSymbol, price, change, timestamp: Date.now() });
          }, 800);
        };

        ch.send({ type: 'init', symbols: Object.keys(SYMBOLS), active: activeSymbol });
        startTicking();

        ch.on('data', (msg) => {
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
    },
  }),
});
