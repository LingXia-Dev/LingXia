import { useChannel, useLocal, type ChannelEndpoint, type View } from '@lingxia/ui';
import '../../theme/index.css';
import { Button, Card, ErrorState, Page, Section } from '../../shared/ui';
import type { ClientCommand, ServerMessage, TickerUpdate } from '../../shared/channel-types';

type Actions = { tickerSession(params: Record<string, never>, channel: ChannelEndpoint<ServerMessage, ClientCommand>): void };
const changeClass = (value: number) => (value < 0 ? 'text-danger' : value > 0 ? 'text-success' : 'text-muted');
const formatChange = (value: number) => `${value > 0 ? '+' : ''}${value.toFixed(2)}`;

export function render({ actions }: View<Actions>) {
  const symbols = useLocal<string[]>('symbols', []);
  const symbol = useLocal('symbol', '');
  const history = useLocal<TickerUpdate[]>('history', []);
  const latest = useLocal<TickerUpdate | null>('latest', null);
  const channel = useChannel('ticker', actions.tickerSession, {
    params: {},
    onData(message) {
      if (message.type === 'init') {
        symbols.set(message.symbols);
        symbol.set(message.active);
        history.set([]);
        latest.set(null);
      } else if (message.symbol === symbol.value) {
        latest.set(message);
        history.set([...history.value.slice(-29), message]);
      }
    },
  });
  const subscribe = (value: string) => {
    if (!channel.connected || value === symbol.value) return;
    channel.send({ type: 'subscribe', symbol: value });
    symbol.set(value);
    history.set([]);
    latest.set(null);
  };
  const status = channel.connected ? 'Connected' : channel.connecting ? 'Connecting...' : 'Disconnected';
  return (
    <Page testId="channel-page">
      <row className="items-center gap-2">
        <spacer
          className={
            channel.connected
              ? 'w-2 h-2 rounded-full bg-success'
              : channel.connecting
                ? 'w-2 h-2 rounded-full bg-warning'
                : 'w-2 h-2 rounded-full bg-line'
          }
        />
        <text testId="channel-status" className="text-xs text-muted">
          {status}
        </text>
      </row>
      <scroll testId="channel-symbols" className="overflow-x-auto">
        <row className="gap-2">
          {symbols.value.map(value => (
            <Button
              key={value}
              size="sm"
              variant={value === symbol.value ? 'primary' : 'secondary'}
              testId={`channel-symbol-${value}`}
              disabled={!channel.connected}
              onClick={() => subscribe(value)}
              className="rounded-full"
            >
              {value}
            </Button>
          ))}
        </row>
      </scroll>
      <Card className="p-6 gap-1">
        <text testId="channel-active" className="text-xs font-medium tracking-wider text-muted">
          {symbol.value || '---'}
        </text>
        <row className="items-baseline gap-3">
          <text testId="channel-price" className="text-4xl font-bold">
            {latest.value ? `$${latest.value.price.toFixed(2)}` : '---'}
          </text>
          {latest.value && (
            <text testId="channel-change" className={`text-lg font-semibold ${changeClass(latest.value.change)}`}>
              {formatChange(latest.value.change)}
            </text>
          )}
        </row>
        {latest.value && (
          <text testId="channel-last-time" className="text-xs text-muted">
            {new Date(latest.value.timestamp).toLocaleTimeString()}
          </text>
        )}
      </Card>
      <Section title="Recent ticks">
        <scroll testId="channel-history" className="max-h-64">
          {!history.value.length ? (
            <text className="py-6 text-sm text-center text-muted">Waiting for data...</text>
          ) : (
            <column className="divide-y divide-line">
              {history.value
                .slice()
                .reverse()
                .map(tick => (
                  <row key={`${tick.timestamp}-${tick.symbol}`} className="items-center justify-between gap-3 py-2.5">
                    <text className="text-sm font-mono">{`$${tick.price.toFixed(2)}`}</text>
                    <row className="items-center gap-3">
                      <text className={`text-sm font-medium ${changeClass(tick.change)}`}>{formatChange(tick.change)}</text>
                      <text className="text-xs text-muted">{new Date(tick.timestamp).toLocaleTimeString()}</text>
                    </row>
                  </row>
                ))}
            </column>
          )}
        </scroll>
      </Section>
      <grid className="gap-3 grid-cols-2">
        <Button variant="secondary" testId="channel-disconnect" disabled={!channel.connected} onClick={() => channel.close()}>
          Disconnect
        </Button>
        <Button testId="channel-reconnect" disabled={channel.connecting || channel.connected} onClick={() => channel.connect()}>
          Reconnect
        </Button>
      </grid>
      <ErrorState error={channel.error} />
    </Page>
  );
}
