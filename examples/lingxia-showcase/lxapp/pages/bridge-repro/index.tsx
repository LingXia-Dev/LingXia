import React, { useEffect, useRef, useState } from 'react';
import { useLxPage, useLxStream } from '@lingxia/react';
import type { BootstrapData, BootstrapProbe, Tick } from './index';
import '../../tailwind.css';

interface AuditState {
  last: number;
  received: number;
  gaps: string[];
  firstAtMs: number | null;
}

const INITIAL_AUDIT: AuditState = { last: 0, received: 0, gaps: [], firstAtMs: null };
const PAGE_START = Date.now();
const BOOTSTRAP_TIMEOUT_MS = 5000;

function isBridgeReady(): boolean {
  const bridge = (window as unknown as {
    LingXiaBridge?: { isReady?: () => boolean };
  }).LingXiaBridge;
  return bridge?.isReady?.() === true;
}

function forceReconnect(): boolean {
  const fn = (window as unknown as Record<string, unknown>).__lxForceDownstreamReconnect;
  if (typeof fn !== 'function') return false;
  (fn as () => void)();
  return true;
}

export default function BridgeReproPage() {
  const { data, actions } = useLxPage<BootstrapData, {
    onBootstrapProbe: (p: { nonce: number; viewSentAt: number }) => Promise<BootstrapProbe>;
    onTicks: () => AsyncGenerator<Tick, void>;
    onEcho: (p: { n: number }) => { n: number; ts: number };
  }>();

  const [bridgeReady, setBridgeReady] = useState(isBridgeReady);
  const [probe, setProbe] = useState<BootstrapProbe | null>(null);
  const [probeError, setProbeError] = useState<string | null>(null);
  const [deadlineReached, setDeadlineReached] = useState(false);
  const [streamError, setStreamError] = useState<string | null>(null);
  const [reconnects, setReconnects] = useState(0);
  const [echoLog, setEchoLog] = useState<string>('');
  const echoSeq = useRef(0);

  const ticks = useLxStream<typeof actions.onTicks, AuditState>(actions.onTicks, {
    manual: true,
    initial: INITIAL_AUDIT,
    reduce: (acc, tick) => {
      const gaps = [...acc.gaps];
      // A restarted generator begins at 1 again; only flag intra-run gaps.
      if (acc.last > 0 && tick.seq > acc.last + 1) {
        gaps.push(`${acc.last + 1}..${tick.seq - 1}`);
      }
      return {
        last: tick.seq,
        received: acc.received + 1,
        gaps,
        firstAtMs: acc.firstAtMs ?? Date.now() - PAGE_START,
      };
    },
  });

  useEffect(() => {
    let active = true;
    const poll = window.setInterval(() => {
      if (active) setBridgeReady(isBridgeReady());
    }, 50);
    const deadline = window.setTimeout(() => {
      if (active) setDeadlineReached(true);
    }, BOOTSTRAP_TIMEOUT_MS);

    const runProbe = async () => {
      try {
        const result = await actions.onBootstrapProbe({ nonce: 1, viewSentAt: PAGE_START });
        if (active) setProbe(result);
      } catch (error) {
        if (active) setProbeError(String((error as Error)?.message ?? error));
      }
    };

    void runProbe();

    return () => {
      active = false;
      window.clearInterval(poll);
      window.clearTimeout(deadline);
    };
  }, [actions]);

  // Watch for stream death: LxStream surfaces errors via the hook's error field.
  useEffect(() => {
    if (ticks.error) setStreamError(String((ticks.error as Error)?.message ?? ticks.error));
  }, [ticks.error]);

  // Drop and reconnect the downstream exactly as WebKit does when it replaces
  // the streaming fetch. The reconnect resumes from the last seq, so a sound
  // transport loses nothing; on the pre-fix transport the stream dies instead.
  const reconnect = () => {
    if (!forceReconnect()) {
      setEchoLog('reconnect hook unavailable (non-Apple transport)');
      return;
    }
    setReconnects((n) => n + 1);
  };

  const echo = async () => {
    const n = ++echoSeq.current;
    const t0 = Date.now();
    try {
      const r = await actions.onEcho({ n });
      setEchoLog(`echo #${r.n} ok in ${Date.now() - t0}ms`);
    } catch (e) {
      setEchoLog(`echo #${n} FAILED: ${String((e as Error)?.message ?? e)}`);
    }
  };

  const audit = ticks.data ?? INITIAL_AUDIT;
  const snapshotReady = data.bootstrapMarker === 'initial logic snapshot received';
  const bootstrapPassed = bridgeReady && snapshotReady && probe !== null;
  const bootstrapFailed = probeError !== null || (deadlineReached && !bootstrapPassed);
  const bootstrapVerdict = bootstrapPassed ? 'PASS' : bootstrapFailed ? 'FAIL' : 'WAITING';
  const bootstrapColor =
    bootstrapVerdict === 'PASS'
      ? 'bg-green-600'
      : bootstrapVerdict === 'FAIL'
        ? 'bg-red-600'
        : 'bg-gray-400';

  const streamFailed = audit.gaps.length > 0 || streamError !== null;
  const streamVerdict = audit.received === 0 ? 'WAITING' : streamFailed ? 'FAIL' : 'PASS';
  const streamColor =
    streamVerdict === 'PASS'
      ? 'bg-green-600'
      : streamVerdict === 'FAIL'
        ? 'bg-red-600'
        : 'bg-gray-400';

  return (
    <div className="min-h-screen p-4 space-y-3 text-sm">
      <div
        className={`rounded-lg px-4 py-3 text-white font-bold text-lg ${bootstrapColor}`}
        id="bootstrap-verdict"
      >
        Bootstrap: {bootstrapVerdict}
      </div>

      <div className="bg-white rounded-lg p-4 space-y-1">
        <div id="bootstrap-ready">bridge ready: {bridgeReady ? 'yes' : 'no'}</div>
        <div id="bootstrap-snapshot">initial snapshot: {snapshotReady ? 'yes' : 'no'}</div>
        <div id="bootstrap-probe">
          logic call:{' '}
          {probe
            ? `yes (${probe.logicReceivedAt - probe.viewSentAt} ms)`
            : probeError
              ? `FAILED: ${probeError}`
              : 'waiting'}
        </div>
        <div id="bootstrap-logic-loaded">
          logic loaded: {snapshotReady ? `${data.logicLoadedAt - PAGE_START} ms` : '-'}
        </div>
        <div className="text-gray-500 text-xs">Restart the lxapp to repeat.</div>
      </div>

      <div
        className={`rounded-lg px-4 py-3 text-white font-bold text-lg ${streamColor}`}
        id="stream-verdict"
      >
        Stream: {streamVerdict}
      </div>

      <div className="bg-white rounded-lg p-4 space-y-1">
        <div id="stat-received">received: {audit.received}</div>
        <div id="stat-last">last seq: {audit.last}</div>
        <div id="stat-first">first tick after: {audit.firstAtMs ?? '-'} ms</div>
        <div id="stat-gaps" className={audit.gaps.length ? 'text-red-600 font-semibold' : ''}>
          gaps: {audit.gaps.length ? audit.gaps.join(', ') : 'none'}
        </div>
        <div id="stat-error" className={streamError ? 'text-red-600 font-semibold' : ''}>
          stream error: {streamError ?? 'none'}
        </div>
        <div id="stat-reconnects">reconnects: {reconnects}</div>
        <div id="stat-echo">{echoLog || 'echo: -'}</div>
      </div>

      <div className="flex gap-2">
        <button
          id="btn-reconnect"
          onClick={reconnect}
          className="flex-1 bg-red-500 text-white rounded-lg py-3 font-semibold active:opacity-70"
        >
          Reconnect
        </button>
        <button
          id="btn-echo"
          onClick={echo}
          className="flex-1 bg-blue-500 text-white rounded-lg py-3 font-semibold active:opacity-70"
        >
          Echo
        </button>
      </div>

      <button
        id="btn-restart"
        onClick={() => {
          setStreamError(null);
          ticks.start();
        }}
        className="w-full bg-gray-700 text-white rounded-lg py-3 font-semibold active:opacity-70"
      >
        {audit.received === 0 ? 'Start stream' : 'Restart stream'}
      </button>

      <p className="text-gray-500 text-xs leading-relaxed">
        Start the stream, then reconnect. Any gap or bridge error fails the check. Echo may time out
        while the long stream occupies AppService; try it while idle.
      </p>
    </div>
  );
}
