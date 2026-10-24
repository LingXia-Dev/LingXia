import { useLocal, useChannel, type View } from '@lingxia/ui';
import '../../theme/index.css';
import type { ServerMessage, ClientCommand, TickerUpdate } from '../../shared/channel-types';

export function render({ env }: View) {
  const symbols = useLocal<string[]>('symbols', []);
  const symbol = useLocal('symbol', '');
  const history = useLocal<TickerUpdate[]>('history', []);
  const latest = useLocal<TickerUpdate | null>('latest', null);
  const channel = useChannel<ServerMessage, ClientCommand>('ticker', 'tickerSession', {
    params: {},
    onData(message) {
      if (message.type === 'init') {
        symbols.set(message.symbols); symbol.set(message.active);
        history.set([]); latest.set(null);
      } else if (message.type === 'tick' && message.symbol === symbol.value) {
        latest.set(message); history.set([...history.value.slice(-29), message]);
      }
    },
  });
  const subscribe = (value: string) => {
    if (!channel.connected || value === symbol.value) return;
    channel.send({ type: 'subscribe', symbol: value });
    symbol.set(value); history.set([]); latest.set(null);
  };
  const height = Math.max(1, env.viewport.height - env.insets.top - Math.max(env.insets.bottom, env.keyboard.height));
  const changeClass = (value: number) => value < 0 ? 'text-red-500 dark:text-red-400' : value > 0 ? 'text-green-600 dark:text-green-400' : 'text-gray-500';
  const formatChange = (value: number) => `${value > 0 ? '+' : ''}${value.toFixed(2)}`;
  return <column testId="channel-page" className="bg-surface-50 text-gray-900">
    <spacer style={{ height: env.insets.top }} />
    <row style={{ height }}><spacer style={{ width: env.insets.left }} />
      <scroll testId="page-scroll" className="grow basis-0 min-w-0" style={{ height }}>
        <column className="px-4 py-6">
          <row className="items-center gap-2 pb-4">
            <column className={channel.connected ? 'w-2 h-2 rounded-full bg-green-500' : channel.connecting ? 'w-2 h-2 rounded-full bg-yellow-400' : 'w-2 h-2 rounded-full bg-surface-300'} />
            <text testId="channel-status" className="text-xs text-gray-500">{channel.connected ? 'Connected' : channel.connecting ? 'Connecting...' : 'Disconnected'}</text>
          </row>
          <scroll testId="channel-symbols" className="overflow-x-auto"><row className="gap-2 pb-6">{symbols.value.map(value => <button key={value} testId={`channel-symbol-${value}`} enabled={channel.connected} onClick={() => subscribe(value)} className={value === symbol.value ? 'shrink-0 px-4 py-2 rounded-full text-sm font-medium bg-blue-600 text-white' : 'shrink-0 px-4 py-2 rounded-full text-sm font-medium bg-surface text-gray-700 border border-line-200'}>{value}</button>)}</row></scroll>
          <column className="bg-surface rounded-2xl shadow-sm border border-line-200 p-6">
            <text testId="channel-active-symbol" className="text-xs font-medium text-gray-400 tracking-wider pb-1">{symbol.value || '---'}</text>
            <row className="items-baseline gap-3">
              <text testId="channel-price" className="text-4xl font-bold">{latest.value ? `$${latest.value.price.toFixed(2)}` : '---'}</text>
              {latest.value ? <text testId="channel-change" className={`text-lg font-semibold ${latest.value.change === 0 ? 'text-gray-600' : changeClass(latest.value.change)}`}>{formatChange(latest.value.change)}</text> : null}
            </row>
            {latest.value ? <text testId="channel-last-time" className="text-xs text-gray-400 pt-1">{new Date(latest.value.timestamp).toLocaleTimeString()}</text> : null}
          </column>
          <spacer className="h-6" />
          <column className="bg-surface rounded-2xl shadow-sm border border-line-200">
            <column className="px-4 py-3 border-b border-line-100"><text className="text-xs font-semibold text-gray-500 tracking-wider">RECENT TICKS</text></column>
            <scroll testId="channel-history" className="max-h-64">
              {history.value.length === 0 ? <text className="px-4 py-6 text-sm text-gray-400 text-center">Waiting for data...</text> : <column>{history.value.slice().reverse().map((tick, index) => <row key={`${tick.timestamp}-${tick.symbol}`} className={index < history.value.length - 1 ? "px-4 py-2.5 items-center justify-between border-b border-line-50 gap-3" : "px-4 py-2.5 items-center justify-between gap-3"}>
                <text className="text-sm text-gray-800 font-mono">{`$${tick.price.toFixed(2)}`}</text>
                <row className="items-center gap-3"><text className={`text-sm font-medium ${changeClass(tick.change)}`}>{formatChange(tick.change)}</text><text className="text-xs text-gray-400">{new Date(tick.timestamp).toLocaleTimeString()}</text></row>
              </row>)}</column>}
            </scroll>
          </column>
          <row className="pt-6 gap-3">
            <button testId="channel-disconnect" enabled={channel.connected} onClick={() => channel.close()} className={channel.connected ? 'grow basis-0 w-0 py-3 rounded-xl text-sm font-medium bg-surface-200 text-gray-700' : 'grow basis-0 w-0 py-3 rounded-xl text-sm font-medium bg-surface-200 text-gray-700 opacity-40'}>Disconnect</button>
            <button testId="channel-reconnect" enabled={!channel.connecting && !channel.connected} onClick={() => channel.connect()} className={!channel.connecting && !channel.connected ? 'grow basis-0 w-0 py-3 rounded-xl text-sm font-medium bg-blue-600 text-white' : 'grow basis-0 w-0 py-3 rounded-xl text-sm font-medium bg-blue-600 text-white opacity-40'}>Reconnect</button>
          </row>
          {channel.error ? <text className="pt-3 text-xs text-red-500 dark:text-red-400 text-center">{`Error: ${channel.error}`}</text> : null}
        </column>
      </scroll><spacer style={{ width: env.insets.right }} />
    </row><spacer style={{ height: Math.max(env.insets.bottom, env.keyboard.height) }} />
  </column>;
}
