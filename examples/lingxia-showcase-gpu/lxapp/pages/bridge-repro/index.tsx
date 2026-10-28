import { useLocal, useRequest, useStream, type View } from '@lingxia/ui';
import '../../theme/index.css';
import { Button, Card, Page } from '../../shared/ui';

type BootstrapProbe = { nonce: number; viewSentAt: number; logicReceivedAt: number; bootstrapMarker: string };
type Tick = { seq: number; ts: number };
type Actions = {
  onBootstrapProbe(params: { nonce: number; viewSentAt: number }): Promise<BootstrapProbe>;
  onTicks(): import('@lingxia/ui').Stream<Tick, void>;
  onEcho(params: { n: number }): Promise<{ n: number; ts: number }>;
};
type Data = { bootstrapMarker?: string; logicLoadedAt?: number };
type Audit = { last: number; received: number; gaps: string[]; firstAtMs: number | null };

const verdictClass = (value: string) =>
  `px-4 py-3 rounded-lg text-lg font-bold ${value === 'PASS' ? 'bg-success text-on-success' : value === 'FAIL' ? 'bg-danger text-on-danger' : 'bg-surface-sunken text-text'}`;

export function render({ data, actions }: View<Actions, Data>) {
  const started = useLocal('started', Date.now());
  const echoNumber = useLocal('echo-number', 0);
  const echoLog = useLocal('echo-log', 'echo: -');
  const echoPending = useLocal('echo-pending', false);
  const probe = useRequest('bootstrap', actions.onBootstrapProbe, { nonce: 1, viewSentAt: started.value });
  const audit = useLocal<Audit>('audit', { last: 0, received: 0, gaps: [], firstAtMs: null });
  const ticks = useStream('ticks', actions.onTicks, {
    onData(tick) {
      const previous = audit.value;
      const gaps =
        previous.last > 0 && tick.seq !== previous.last + 1 ? [...previous.gaps.slice(-31), `${previous.last} → ${tick.seq}`] : previous.gaps;
      audit.set({ last: tick.seq, received: previous.received + 1, gaps, firstAtMs: previous.firstAtMs ?? Date.now() - started.value });
    },
  });
  const snapshotReady = data.bootstrapMarker === 'initial logic snapshot received';
  const passed = snapshotReady && probe.value?.nonce === 1 && probe.value.bootstrapMarker === data.bootstrapMarker;
  const echo = async () => {
    if (echoPending.value) return;
    const n = echoNumber.value + 1,
      sent = Date.now();
    echoNumber.set(n);
    echoPending.set(true);
    try {
      const result = await actions.onEcho({ n });
      echoLog.set(result.n === n ? `echo #${n} ok in ${Date.now() - sent}ms` : `echo #${n} FAILED: nonce mismatch`);
    } catch (error) {
      echoLog.set(`echo #${n} FAILED: ${String(error)}`);
    } finally {
      echoPending.set(false);
    }
  };
  const bootstrapVerdict = passed ? 'PASS' : probe.error ? 'FAIL' : 'WAITING';
  const streamVerdict = audit.value.gaps.length || ticks.error ? 'FAIL' : audit.value.received ? 'PASS' : 'WAITING';
  const loaded = Number(data.logicLoadedAt);
  return (
    <Page testId="bridge-repro-page" data={{ 'data-automation-contract': 'bridge-v1' }}>
      <text testId="bootstrap-verdict" className={verdictClass(bootstrapVerdict)}>{`Bootstrap: ${bootstrapVerdict}`}</text>
      <Card className="p-4 gap-1">
        <text testId="bootstrap-ready" className="text-sm">{`bridge ready: ${probe.value ? 'yes' : 'no'}`}</text>
        <text testId="bootstrap-snapshot" className="text-sm">{`initial snapshot: ${snapshotReady ? 'yes' : 'no'}`}</text>
        <text testId="bootstrap-probe" className="text-sm">
          {probe.value
            ? `logic call: yes (${probe.value.logicReceivedAt - probe.value.viewSentAt} ms)`
            : probe.error
              ? `logic call: FAILED: ${probe.error.message}`
              : 'logic call: waiting'}
        </text>
        <text testId="bootstrap-logic-loaded" className="text-sm">
          {snapshotReady ? `logic ready: ${loaded <= started.value ? 'preloaded' : `${loaded - started.value} ms after view`}` : 'logic ready: -'}
        </text>
        <text className="text-xs text-muted">Restart the lxapp to repeat.</text>
      </Card>
      <text testId="stream-verdict" className={verdictClass(streamVerdict)}>{`Stream: ${streamVerdict}`}</text>
      <Card className="p-4 gap-1">
        <text testId="stat-received" className="text-sm">{`received: ${audit.value.received}`}</text>
        <text testId="stat-last" className="text-sm">{`last seq: ${audit.value.last}`}</text>
        <text testId="stat-first" className="text-sm">{`first tick after: ${audit.value.firstAtMs ?? '-'} ms`}</text>
        <text
          testId="stat-gaps"
          className={audit.value.gaps.length ? 'text-sm font-semibold text-danger' : 'text-sm'}
        >{`gaps: ${audit.value.gaps.join(', ') || 'none'}`}</text>
        <text
          testId="stat-error"
          className={ticks.error ? 'text-sm font-semibold text-danger' : 'text-sm'}
        >{`stream error: ${ticks.error?.message || 'none'}`}</text>
        <text testId="stat-reconnects" className="text-sm">
          reconnects: 0
        </text>
        <text testId="stat-echo" className="text-sm">
          {echoLog.value}
        </text>
      </Card>
      <grid className="gap-2 grid-cols-2">
        <Button variant="destructive" testId="btn-reconnect" onClick={() => echoLog.set('reconnect hook unavailable (GPU transport)')}>
          Reconnect
        </Button>
        <Button testId="btn-echo" disabled={echoPending.value} onClick={echo}>
          Echo
        </Button>
        <Button
          variant="secondary"
          testId="btn-restart"
          onClick={() => {
            audit.set({ last: 0, received: 0, gaps: [], firstAtMs: null });
            ticks.start();
          }}
        >
          {audit.value.received ? 'Restart stream' : 'Start stream'}
        </Button>
        <Button variant="secondary" testId="btn-stop" onClick={() => ticks.cancel()}>
          Stop
        </Button>
      </grid>
      <text className="text-xs text-muted">
        Start the stream, then reconnect. Any gap or bridge error fails the check. Echo may time out while the long stream occupies AppService.
      </text>
    </Page>
  );
}
