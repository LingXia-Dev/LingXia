import React, { useState, useRef, useEffect } from 'react';
import { useLxPage, useLxChannel } from '@lingxia/react';
import type { LxChannel } from '@lingxia/bridge';
import type { ServerMessage, ClientCommand, TickerUpdate } from './index';
import '../../tailwind.css';

interface TickerSnapshot {
  symbol: string;
  price: number;
  change: number;
  timestamp: number;
}

export default function ChannelPage() {
  const { actions } = useLxPage<
    { connected: boolean },
    {
      tickerSession: (params: Record<string, unknown>) => Promise<LxChannel<ServerMessage, ClientCommand>>;
    }
  >();

  const session = useLxChannel(actions.tickerSession, {
    params: () => ({}),
  });

  const [symbols, setSymbols] = useState<string[]>([]);
  const [active, setActive] = useState('');
  const [history, setHistory] = useState<TickerSnapshot[]>([]);
  const [latest, setLatest] = useState<TickerSnapshot | null>(null);
  const historyRef = useRef(history);
  historyRef.current = history;

  useEffect(() => {
    if (!session.last) return;
    const msg = session.last as ServerMessage;
    if (msg.type === 'init') {
      setSymbols(msg.symbols);
      setActive(msg.active);
      setHistory([]);
      setLatest(null);
    }
    if (msg.type === 'tick') {
      const snap: TickerSnapshot = {
        symbol: msg.symbol,
        price: msg.price,
        change: msg.change,
        timestamp: msg.timestamp,
      };
      setLatest(snap);
      setHistory((prev) => [...prev.slice(-29), snap]);
    }
  }, [session.last]);

  const switchSymbol = (symbol: string) => {
    if (symbol === active || !session.connected) return;
    setActive(symbol);
    setHistory([]);
    setLatest(null);
    session.send({ type: 'subscribe', symbol });
  };

  const changeColor = latest
    ? latest.change > 0
      ? 'text-green-600'
      : latest.change < 0
        ? 'text-red-500'
        : 'text-gray-600'
    : 'text-gray-600';

  const changePrefix = latest && latest.change > 0 ? '+' : '';

  return (
    <div className="min-h-screen bg-gray-50 px-4 py-6">
      {/* Connection status */}
      <div className="flex items-center gap-2 mb-4">
        <div
          className={`w-2 h-2 rounded-full ${
            session.connected ? 'bg-green-500' : session.connecting ? 'bg-yellow-400' : 'bg-gray-300'
          }`}
        />
        <span className="text-xs text-gray-500">
          {session.connected ? 'Connected' : session.connecting ? 'Connecting...' : 'Disconnected'}
        </span>
      </div>

      {/* Symbol tabs */}
      <div className="flex gap-2 mb-6 overflow-x-auto">
        {symbols.map((sym) => (
          <button
            key={sym}
            onClick={() => switchSymbol(sym)}
            className={`px-4 py-2 rounded-full text-sm font-medium transition-colors ${
              sym === active
                ? 'bg-blue-600 text-white'
                : 'bg-white text-gray-700 border border-gray-200'
            }`}
          >
            {sym}
          </button>
        ))}
      </div>

      {/* Price card */}
      <div className="bg-white rounded-2xl shadow-sm border border-gray-200 p-6 mb-6">
        <p className="text-xs font-medium text-gray-400 uppercase tracking-wider mb-1">{active || '---'}</p>
        <div className="flex items-baseline gap-3">
          <span className="text-4xl font-bold text-gray-900">
            {latest ? `$${latest.price.toFixed(2)}` : '---'}
          </span>
          {latest && (
            <span className={`text-lg font-semibold ${changeColor}`}>
              {changePrefix}{latest.change.toFixed(2)}
            </span>
          )}
        </div>
        {latest && (
          <p className="text-xs text-gray-400 mt-1">
            {new Date(latest.timestamp).toLocaleTimeString()}
          </p>
        )}
      </div>

      {/* Tick history */}
      <div className="bg-white rounded-2xl shadow-sm border border-gray-200 overflow-hidden">
        <div className="px-4 py-3 border-b border-gray-100">
          <p className="text-xs font-semibold text-gray-500 uppercase tracking-wider">Recent Ticks</p>
        </div>
        <div className="max-h-64 overflow-y-auto">
          {history.length === 0 ? (
            <p className="px-4 py-6 text-sm text-gray-400 text-center">Waiting for data...</p>
          ) : (
            <div className="divide-y divide-gray-50">
              {[...history].reverse().map((tick, i) => {
                const color =
                  tick.change > 0 ? 'text-green-600' : tick.change < 0 ? 'text-red-500' : 'text-gray-500';
                const prefix = tick.change > 0 ? '+' : '';
                return (
                  <div key={i} className="px-4 py-2.5 flex items-center justify-between">
                    <span className="text-sm text-gray-800 font-mono">${tick.price.toFixed(2)}</span>
                    <div className="flex items-center gap-3">
                      <span className={`text-sm font-medium ${color}`}>
                        {prefix}{tick.change.toFixed(2)}
                      </span>
                      <span className="text-xs text-gray-400">
                        {new Date(tick.timestamp).toLocaleTimeString()}
                      </span>
                    </div>
                  </div>
                );
              })}
            </div>
          )}
        </div>
      </div>

      {/* Actions */}
      <div className="mt-6 flex gap-3">
        <button
          onClick={() => session.close()}
          disabled={!session.connected}
          className="flex-1 py-3 rounded-xl text-sm font-medium bg-gray-200 text-gray-700 disabled:opacity-40"
        >
          Disconnect
        </button>
        <button
          onClick={() => session.reopen()}
          disabled={session.connected || session.connecting}
          className="flex-1 py-3 rounded-xl text-sm font-medium bg-blue-600 text-white disabled:opacity-40"
        >
          Reconnect
        </button>
      </div>

      {session.error && (
        <p className="mt-3 text-xs text-red-500 text-center">
          Error: {session.error.code} {session.error.message && `\u2014 ${session.error.message}`}
        </p>
      )}
    </div>
  );
}
