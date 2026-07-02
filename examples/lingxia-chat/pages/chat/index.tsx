import React, { useState, useRef, useEffect } from 'react';
import * as echarts from 'echarts';
import { useLxPage, useLxStream } from '@lingxia/react';
import type { LxStream } from '@lingxia/bridge';
import type { Message, ChatChunk, ChartData } from './index';
import '../../tailwind.css';

const PALETTE = ['#3b82f6', '#8b5cf6', '#10b981', '#f59e0b', '#ef4444', '#06b6d4'];

function buildOption(data: ChartData): echarts.EChartsOption {
  const labels = data.series.map((s) => s.label);
  const values = data.series.map((s) => s.value);

  if (data.kind === 'pie') {
    return {
      color: PALETTE,
      tooltip: { trigger: 'item', formatter: '{b}: {d}%' },
      legend: {
        bottom: 0,
        textStyle: { fontSize: 11, color: '#6b7280' },
        icon: 'circle',
        itemWidth: 8,
        itemHeight: 8,
        itemGap: 12,
      },
      series: [
        {
          type: 'pie',
          radius: ['40%', '68%'],
          center: ['50%', '44%'],
          data: data.series.map((s) => ({ name: s.label, value: s.value })),
          label: { show: false },
          emphasis: {
            label: { show: true, fontSize: 13, fontWeight: 'bold' as const },
            scale: true,
            scaleSize: 5,
          },
          animationType: 'scale',
          animationEasing: 'elasticOut' as const,
        },
      ],
    };
  }

  const isLine = data.kind === 'line';
  return {
    color: PALETTE,
    grid: { top: 10, right: 10, bottom: 24, left: 10, containLabel: true },
    tooltip: { trigger: 'axis', axisPointer: { type: 'shadow' } },
    xAxis: {
      type: 'category',
      data: labels,
      axisLine: { lineStyle: { color: '#e5e7eb' } },
      axisTick: { show: false },
      axisLabel: { fontSize: 11, color: '#6b7280' },
    },
    yAxis: {
      type: 'value',
      splitLine: { lineStyle: { color: '#f3f4f6', type: 'dashed' } },
      axisLabel: { fontSize: 11, color: '#6b7280' },
      axisLine: { show: false },
      axisTick: { show: false },
    },
    series: [
      {
        type: isLine ? 'line' : 'bar',
        data: values,
        smooth: isLine ? 0.4 : false,
        symbolSize: isLine ? 6 : undefined,
        areaStyle: isLine
          ? {
              color: new echarts.graphic.LinearGradient(0, 0, 0, 1, [
                { offset: 0, color: 'rgba(59,130,246,0.22)' },
                { offset: 1, color: 'rgba(59,130,246,0)' },
              ]),
            }
          : undefined,
        lineStyle: isLine ? { width: 2.5 } : undefined,
        itemStyle: { borderRadius: isLine ? undefined : [4, 4, 0, 0] },
        barMaxWidth: 36,
      },
    ],
  };
}

function ChartCard({ data }: { data: ChartData }) {
  const containerRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!containerRef.current) return;
    const chart = echarts.init(containerRef.current, null, { renderer: 'svg' });
    chart.setOption(buildOption(data));
    return () => chart.dispose();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const height = data.kind === 'pie' ? 210 : 180;

  return (
    <div className="mt-3 rounded-2xl overflow-hidden bg-gray-50 border border-gray-200 shadow-sm animate-chart-in">
      <p className="text-[10px] font-semibold tracking-widest uppercase text-gray-400 px-3.5 pt-3 pb-0.5">
        {data.title}
      </p>
      <div ref={containerRef} style={{ width: '100%', height }} />
    </div>
  );
}

interface StreamState {
  text: string;
  chart?: ChartData;
}

const HINTS = [
  'Tell me about LingXia streaming',
  'Show me some data',
  'How does the bridge protocol work?',
];

function EmptyState() {
  return (
    <div className="flex-1 flex flex-col items-center justify-center gap-3 px-8 text-center">
      <div className="w-16 h-16 rounded-2xl bg-white shadow flex items-center justify-center">
        <svg viewBox="0 0 24 24" fill="none" className="w-8 h-8" stroke="#2563EB" strokeWidth="1.5">
          <path
            strokeLinecap="round"
            strokeLinejoin="round"
            d="M8.625 12a.375.375 0 11-.75 0 .375.375 0 01.75 0zm0 0H8.25m4.125 0a.375.375 0 11-.75 0 .375.375 0 01.75 0zm0 0H12m4.125 0a.375.375 0 11-.75 0 .375.375 0 01.75 0zm0 0h-.375M21 12c0 4.556-4.03 8.25-9 8.25a9.764 9.764 0 01-2.555-.337A5.972 5.972 0 015.41 20.97a5.969 5.969 0 01-.474-.065 4.48 4.48 0 00.978-2.025c.09-.457-.133-.901-.467-1.226C3.93 16.178 3 14.189 3 12c0-4.556 4.03-8.25 9-8.25s9 3.694 9 8.25z"
          />
        </svg>
      </div>
      <div>
        <p className="text-base font-semibold text-gray-800">AI Chat</p>
        <p className="text-sm text-gray-500 mt-1">
          Streaming demo — text &amp; chart artifacts via LingXia bridge.
        </p>
      </div>
      <div className="flex flex-col gap-2 w-full mt-2">
        {HINTS.map((hint) => (
          <div
            key={hint}
            className="text-sm text-blue-600 bg-blue-50 rounded-xl px-4 py-2.5 text-left"
          >
            {hint}
          </div>
        ))}
      </div>
    </div>
  );
}

// Desktop hosts (macOS/Windows) declare a dockable terminal surface; the
// bridge global is the View-side platform source.
function hasDesktopTerminal(): boolean {
  try {
    const p = (window as unknown as {
      LingXiaBridge?: { platform?: { isMacOS(): boolean; isWindows(): boolean } };
    }).LingXiaBridge?.platform;
    return !!p && (p.isMacOS() || p.isWindows());
  } catch {
    return false;
  }
}

const TERMINAL_EDGES = [
  { edge: 'left' as const, label: 'Dock left', arrow: 'M15 6l-6 6 6 6' },
  { edge: 'right' as const, label: 'Dock right', arrow: 'M9 6l6 6-6 6' },
  { edge: 'top' as const, label: 'Dock top', arrow: 'M6 15l6-6 6 6' },
  { edge: 'bottom' as const, label: 'Dock bottom', arrow: 'M6 9l6 6 6-6' },
];

function TerminalTool({ onOpen }: { onOpen: (edge: 'left' | 'right' | 'top' | 'bottom') => void }) {
  const [open, setOpen] = useState(false);

  return (
    <div className="relative">
      <button
        onClick={() => setOpen((v) => !v)}
        title="Terminal"
        className="w-7 h-7 bg-white rounded-full shadow-sm flex items-center justify-center active:opacity-70"
      >
        <svg viewBox="0 0 24 24" fill="none" className="w-4 h-4" stroke="#374151" strokeWidth="1.8">
          <path strokeLinecap="round" strokeLinejoin="round" d="M7 9l3 3-3 3m5 0h5M4.5 5h15a1 1 0 011 1v12a1 1 0 01-1 1h-15a1 1 0 01-1-1V6a1 1 0 011-1z" />
        </svg>
      </button>
      {open && (
        <>
          <div className="fixed inset-0 z-10" onClick={() => setOpen(false)} />
          <div className="absolute right-0 top-9 z-20 bg-white rounded-xl shadow-lg border border-gray-200 py-1 w-36">
            {TERMINAL_EDGES.map(({ edge, label, arrow }) => (
              <button
                key={edge}
                onClick={() => {
                  setOpen(false);
                  onOpen(edge);
                }}
                className="w-full flex items-center gap-2 px-3 py-2 text-sm text-gray-700 hover:bg-gray-50 active:bg-gray-100"
              >
                <svg viewBox="0 0 24 24" fill="none" className="w-3.5 h-3.5" stroke="#6b7280" strokeWidth="2">
                  <path strokeLinecap="round" strokeLinejoin="round" d={arrow} />
                </svg>
                {label}
              </button>
            ))}
          </div>
        </>
      )}
    </div>
  );
}

function AIAvatar() {
  return (
    <div className="w-7 h-7 rounded-full bg-gradient-to-br from-violet-500 to-blue-600 flex-shrink-0 flex items-center justify-center mt-0.5">
      <svg viewBox="0 0 24 24" fill="white" className="w-3.5 h-3.5">
        <path d="M12 2a10 10 0 110 20A10 10 0 0112 2zm0 2a8 8 0 100 16A8 8 0 0012 4zm-1 5h2v2h-2V9zm0 4h2v6h-2v-6z" />
      </svg>
    </div>
  );
}

function MessageBubble({ message }: { message: Message }) {
  if (message.role === 'user') {
    return (
      <div className="flex justify-end">
        <div
          className="max-w-[78%] px-4 py-2.5 rounded-3xl rounded-br-md bg-blue-600 text-white text-sm leading-relaxed"
          style={{ wordBreak: 'break-word' }}
        >
          {message.content}
        </div>
      </div>
    );
  }

  return (
    <div className="flex justify-start">
      <div className="flex items-start gap-2 max-w-[90%]">
        <AIAvatar />
        <div
          className="px-4 py-2.5 rounded-3xl rounded-bl-md bg-white border border-gray-200 text-gray-800 text-sm leading-relaxed shadow-sm"
          style={{ wordBreak: 'break-word' }}
        >
          {message.content || <span className="text-gray-400 italic">...</span>}
          {message.chart && <ChartCard data={message.chart} />}
        </div>
      </div>
    </div>
  );
}

function StreamingBubble({ state }: { state: StreamState }) {
  const showCursor = !state.chart;

  return (
    <div className="flex justify-start">
      <div className="flex items-start gap-2 max-w-[90%]">
        <AIAvatar />
        <div
          className="px-4 py-2.5 rounded-3xl rounded-bl-md bg-white border border-gray-200 text-gray-800 text-sm leading-relaxed shadow-sm"
          style={{ wordBreak: 'break-word' }}
        >
          {state.text ? (
            <>
              {state.text}
              {showCursor && (
                <span className="inline-block w-0.5 h-[1.1em] bg-blue-500 ml-0.5 align-middle animate-blink" />
              )}
            </>
          ) : (
            <span className="text-gray-400 italic">
              ...
              <span className="inline-block w-0.5 h-[1.1em] bg-blue-400 ml-0.5 align-middle animate-blink" />
            </span>
          )}
          {state.chart && <ChartCard data={state.chart} />}
        </div>
      </div>
    </div>
  );
}

function InputBar({
  value,
  onChange,
  onSend,
  onStop,
  streaming,
}: {
  value: string;
  onChange: (v: string) => void;
  onSend: () => void;
  onStop: () => void;
  streaming: boolean;
}) {
  const textareaRef = useRef<HTMLTextAreaElement>(null);

  const autoResize = () => {
    const el = textareaRef.current;
    if (!el) return;
    el.style.height = 'auto';
    el.style.height = Math.min(el.scrollHeight, 120) + 'px';
  };

  const handleKeyDown = (e: React.KeyboardEvent<HTMLTextAreaElement>) => {
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault();
      onSend();
    }
  };

  return (
    <div
      className="bg-white border-t border-gray-200 px-3 py-3 flex items-end gap-2"
      style={{ paddingBottom: 'max(12px, env(safe-area-inset-bottom))' }}
    >
      <div className="flex-1 bg-gray-100 rounded-2xl px-3.5 py-2.5 flex items-end gap-2">
        <textarea
          ref={textareaRef}
          value={value}
          onChange={(e) => { onChange(e.target.value); autoResize(); }}
          onKeyDown={handleKeyDown}
          placeholder="Message..."
          rows={1}
          disabled={streaming}
          className="flex-1 bg-transparent text-sm text-gray-800 placeholder-gray-400 outline-none resize-none leading-relaxed"
          style={{ maxHeight: '120px', minHeight: '22px' }}
        />
      </div>

      {streaming ? (
        <button
          onClick={onStop}
          className="w-9 h-9 flex-shrink-0 rounded-full bg-gray-800 flex items-center justify-center active:opacity-70"
        >
          <div className="w-3 h-3 bg-white rounded-sm" />
        </button>
      ) : (
        <button
          onClick={onSend}
          disabled={!value.trim()}
          className="w-9 h-9 flex-shrink-0 rounded-full bg-blue-600 flex items-center justify-center active:opacity-70 disabled:opacity-30 disabled:cursor-not-allowed"
        >
          <svg viewBox="0 0 24 24" fill="white" className="w-4 h-4" style={{ marginBottom: '1px' }}>
            <path d="M12 4l8 8H14v8h-4v-8H4l8-8z" />
          </svg>
        </button>
      )}
    </div>
  );
}

export default function ChatPage() {
  const { data, actions } = useLxPage<
    { messages: Message[] },
    {
      onSend: (params: { text: string }) => LxStream<ChatChunk, void>;
      onClear: () => void;
      onOpenTerminal: (params: { edge: 'left' | 'right' | 'top' | 'bottom' }) => void;
    }
  >();

  const messages = data?.messages ?? [];
  const [inputText, setInputText] = useState('');
  const scrollRef = useRef<HTMLDivElement>(null);

  const chat = useLxStream<typeof actions.onSend, StreamState>(
    actions.onSend,
    {
      params: () => ({ text: inputText }),
      manual: true,
      initial: { text: '' },
      reduce: (acc, chunk) => {
        if (chunk.type === 'token')    return { ...acc, text: acc.text + chunk.token };
        if (chunk.type === 'artifact') return { ...acc, chart: chunk.chart };
        return acc;
      },
    },
  );

  useEffect(() => {
    const el = scrollRef.current;
    if (el) el.scrollTop = el.scrollHeight;
  }, [messages, chat.data]);

  const handleSend = () => {
    const text = inputText.trim();
    if (!text || chat.streaming) return;
    setInputText('');
    chat.start();
  };

  const streamState = chat.data ?? { text: '' };

  return (
    <div className="flex flex-col bg-gray-100" style={{ height: '100vh' }}>
      <div className="absolute top-3 right-4 z-10 flex items-center gap-2">
        {messages.length > 0 && !chat.streaming && (
          <button
            onClick={() => actions.onClear()}
            className="text-xs text-blue-600 px-3 py-1 bg-white rounded-full shadow-sm active:opacity-70"
          >
            Clear
          </button>
        )}
        {hasDesktopTerminal() && (
          <TerminalTool onOpen={(edge) => actions.onOpenTerminal({ edge })} />
        )}
      </div>

      <div ref={scrollRef} className="flex-1 overflow-y-auto px-4 py-4">
        {messages.length === 0 && !chat.streaming ? (
          <EmptyState />
        ) : (
          <div className="flex flex-col gap-3">
            {messages.map((msg) => (
              <MessageBubble key={msg.id} message={msg} />
            ))}
            {chat.streaming && <StreamingBubble state={streamState} />}
          </div>
        )}
      </div>

      <InputBar
        value={inputText}
        onChange={setInputText}
        onSend={handleSend}
        onStop={() => chat.cancel()}
        streaming={chat.streaming}
      />
    </div>
  );
}
