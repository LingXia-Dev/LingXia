import { useLocal, useRequest, useStream, type View } from '@lingxia/ui';
import '../../theme/index.css';
type BootstrapProbe = { nonce: number; viewSentAt: number; logicReceivedAt: number; bootstrapMarker: string };

export function render({ data, actions, env }: View) {
  const started = useLocal('started', Date.now());
  const echoNumber = useLocal('echo-number', 0);
  const echoLog = useLocal('echo-log', 'echo: -');
  const echoPending = useLocal('echo-pending', false);
  const probe = useRequest<BootstrapProbe>('bootstrap', 'onBootstrapProbe', { nonce: 1, viewSentAt: started.value });
  const audit = useLocal<{ last: number; received: number; gaps: string[]; firstAtMs: number | null }>('audit', { last: 0, received: 0, gaps: [], firstAtMs: null });
  const ticks = useStream<{ seq: number; ts: number }>('ticks', 'onTicks', {
    onData(tick) {
      const previous = audit.value;
      const gaps = previous.last > 0 && tick.seq !== previous.last + 1
        ? [...previous.gaps.slice(-31), `${previous.last} → ${tick.seq}`] : previous.gaps;
      audit.set({ last: tick.seq, received: previous.received + 1, gaps, firstAtMs: previous.firstAtMs ?? Date.now() - started.value });
    },
  });
  const snapshotReady = data.bootstrapMarker === 'initial logic snapshot received';
  const passed = snapshotReady && probe.value?.nonce === 1 && probe.value.bootstrapMarker === data.bootstrapMarker;
  const echo = async () => {
    if (echoPending.value) return;
    const n = echoNumber.value + 1;
    echoNumber.set(n); echoPending.set(true);
    const sent = Date.now();
    try {
      const result = await actions.onEcho({ n }) as { n: number; ts: number };
      echoLog.set(result.n === n ? `echo #${n} ok in ${Date.now() - sent}ms` : `echo #${n} FAILED: nonce mismatch`);
    } catch (error) { echoLog.set(`echo #${n} FAILED: ${String(error)}`); }
    finally { echoPending.set(false); }
  };
  const height = Math.max(1, env.viewport.height - env.insets.top - Math.max(env.insets.bottom, env.keyboard.height));
  const bootstrapVerdict = passed ? 'PASS' : probe.error ? 'FAIL' : 'WAITING';
  const streamVerdict = audit.value.gaps.length || ticks.error ? 'FAIL' : audit.value.received ? 'PASS' : 'WAITING';
  const verdictClass = (value: string) => value === 'PASS' ? 'rounded-lg px-4 py-3 text-white font-bold text-lg bg-green-600' : value === 'FAIL' ? 'rounded-lg px-4 py-3 text-white font-bold text-lg bg-red-600' : 'rounded-lg px-4 py-3 text-white font-bold text-lg bg-surface-400';
  return <column testId="bridge-repro-page" className="bg-surface-100 text-gray-900 text-sm">
    <spacer style={{ height: env.insets.top }} />
    <row style={{ height }}><spacer style={{ width: env.insets.left }} />
      <scroll testId="page-scroll" className="grow basis-0 min-w-0" style={{ height }}>
        <column className="p-4 gap-3">
          <text testId="bootstrap-verdict" className={verdictClass(bootstrapVerdict)}>{`Bootstrap: ${bootstrapVerdict}`}</text>
          <column className="bg-surface rounded-lg p-4 gap-1">
            <text testId="bootstrap-ready">bridge ready: {probe.value ? 'yes' : 'no'}</text>
            <text testId="bootstrap-snapshot">initial snapshot: {snapshotReady ? 'yes' : 'no'}</text>
            <text testId="bootstrap-probe">{probe.value ? `logic call: yes (${probe.value.logicReceivedAt - probe.value.viewSentAt} ms)` : probe.error ? `logic call: FAILED: ${probe.error}` : 'logic call: waiting'}</text>
            <text testId="bootstrap-logic-loaded">{snapshotReady ? `logic ready: ${Number(data.logicLoadedAt) <= started.value ? 'preloaded' : `${Number(data.logicLoadedAt) - started.value} ms after view`}` : 'logic ready: -'}</text>
            <text className="text-gray-500 text-xs">Restart the lxapp to repeat.</text>
          </column>
          <text testId="stream-verdict" className={verdictClass(streamVerdict)}>{`Stream: ${streamVerdict}`}</text>
          <column className="bg-surface rounded-lg p-4 gap-1">
            <text testId="stat-received">{`received: ${audit.value.received}`}</text>
            <text testId="stat-last">{`last seq: ${audit.value.last}`}</text>
            <text testId="stat-first">{`first tick after: ${audit.value.firstAtMs ?? '-'} ms`}</text>
            <text testId="stat-gaps" className={audit.value.gaps.length ? 'text-red-600 dark:text-red-400 font-semibold' : ''}>{`gaps: ${audit.value.gaps.join(', ') || 'none'}`}</text>
            <text testId="stat-error" className={ticks.error ? 'text-red-600 dark:text-red-400 font-semibold' : ''}>{`stream error: ${ticks.error || 'none'}`}</text>
            <text testId="stat-reconnects">reconnects: 0</text>
            <text testId="stat-echo">{echoLog.value}</text>
          </column>
          <row className="gap-2">
            <button testId="btn-reconnect" onClick={() => echoLog.set('reconnect hook unavailable (GPU transport)')} className="grow basis-0 w-0 bg-red-500 text-white rounded-lg py-3 font-semibold">Reconnect</button>
            <button testId="btn-echo" enabled={!echoPending.value} onClick={echo} className="grow basis-0 w-0 bg-blue-500 text-white rounded-lg py-3 font-semibold">Echo</button>
          </row>
          <row className="gap-2">
            <button testId="btn-restart" onClick={() => { audit.set({ last: 0, received: 0, gaps: [], firstAtMs: null }); ticks.start(); }} className="grow basis-0 min-w-0 bg-surface-700 text-white rounded-lg py-3 font-semibold">{audit.value.received ? 'Restart stream' : 'Start stream'}</button>
            <button testId="btn-stop" onClick={() => ticks.cancel()} className="bg-surface-500 px-5 text-white rounded-lg py-3 font-semibold">Stop</button>
          </row>
          <text className="text-gray-500 text-xs leading-relaxed">Start the stream, then reconnect. Any gap or bridge error fails the check. Echo may time out while the long stream occupies AppService; try it while idle.</text>
        </column>
      </scroll><spacer style={{ width: env.insets.right }} />
    </row><spacer style={{ height: Math.max(env.insets.bottom, env.keyboard.height) }} />
  </column>;
}
