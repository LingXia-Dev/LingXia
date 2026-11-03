// h-app pages/speedtest (views/view.tsx + components/speedtest-content.tsx), ported construct for
// construct: the 270° gauge with ticks, the three-step ring with a gradient past segment, live
// readings, the verdict and figures. Classes are h-app's; DOM elements map to View elements, an
// SVG becomes one <vector>, and `speedtest-orbit` (a conic gradient behind a radial mask) is a
// conic-gradient stroke.
import type { View, VectorPath } from '@lingxia/view';
import '../../theme/heights.css';
import { Badge, Button, Icon, cn } from '../../shared/ui';
import type { IconName } from '../../shared/icons';

type Phase = 'preparing' | 'ping' | 'download' | 'upload';
type Reading = { phase?: Phase; latencyMs?: number | null; downMbps?: number | null; upMbps?: number | null };
type Result = { downMbps: number | null; upMbps: number | null; latencyMs: number | null; jitterMs: number | null; lossPct: number | null };
type Data = {
  requestStatus: 'ready' | 'running' | 'success' | 'error';
  previous: { downMbps: number; upMbps: number | null; ago: string } | null;
  reading: Reading | null;
  elapsed: number;
  result: Result | null;
};
type Actions = { start(): void; cancel(): void };

const en = {
  route: 'Home internet', startShort: 'Start', start: 'Start speed test', durationHint: 'Usually takes a minute or two.',
  lastTest: 'Last test', atRouterTitle: 'Measured at your router',
  atRouterBody: 'This tests the internet line into your home, not this phone’s Wi‑Fi. Others at home may notice a brief slowdown while it runs.',
  measuresTitle: 'What we measure', pingHint: 'How fast the network responds', downloadHint: 'How fast pages, calls and video arrive',
  uploadHint: 'How fast you send files and video', keepOpen: 'Keep this screen open while we measure. You can cancel at any time.',
  slowHint: 'Still measuring. Slower lines can take a few minutes.', cancel: 'Cancel', testing: 'Starting the test…',
  preparing: 'Reaching the test server…', pingPhase: 'Measuring latency', downloadPhase: 'Testing download', uploadPhase: 'Testing upload',
  stepOf: 'Step {n} of 3', elapsed: 'Elapsed', inProgress: 'In progress', stepDone: 'Done', ping: 'Ping', download: 'Download', upload: 'Upload',
  healthy: 'Healthy', ok: 'Fair', issues: 'Needs attention', partial: 'Partly measured', notMeasured: 'Not measured',
  handlesTitle: 'What this connection handles', use4k: '4K streaming', useCalls: 'Video calls', useGaming: 'Online gaming', useUploads: 'Large uploads',
  useOk: 'Smooth', useWeak: 'May struggle', useUnrated: 'Not rated', runAgain: 'Run again',
};
type Key = keyof typeof en;
const t = (key: Key, values: Record<string, string | number> = {}) => en[key].replace(/\{(\w+)\}/g, (_, name) => String(values[name]));

const CARD = 'rounded-lg border border-border bg-card shadow-[var(--card-shadow)]';
const EYEBROW = 'text-caption font-medium uppercase tracking-wide text-muted-foreground';

function Shell({ children }: { children: (JSX.Element | null)[] }) {
  return (
    <page className="density-compact bg-background text-body text-foreground">
      <scroll className="grow">
        <column className="mx-auto min-h-full w-full max-w-md flex-col px-gutter pb-[max(1.5rem,env(safe-area-inset-bottom))] pt-4">{children}</column>
      </scroll>
    </page>
  );
}

function HeroCard({ aside, children }: { aside: JSX.Element; children: (JSX.Element | null)[] }) {
  return (
    <column className={`speedtest-hero relative overflow-hidden p-4 ${CARD}`}>
      <view className="brand-gradient absolute inset-x-0 top-0 h-0.5" aria-hidden />
      <row className="min-h-6 items-center justify-between gap-2">{aside}</row>
      {children}
    </column>
  );
}

function RouteLabel() {
  return (
    <row className="min-w-0 items-center gap-1.5">
      <Icon name="Router" size={13} className="text-muted-foreground" />
      <text className={`truncate ${EYEBROW}`}>{t('route')}</text>
    </row>
  );
}

const RING_SIZE = 240, RING_SWEEP = 0.75, RING_ROTATION = 135;
function ringGeometry(stroke: number) {
  const c = RING_SIZE / 2;
  const radius = (RING_SIZE - stroke) / 2 - 4;
  const circumference = 2 * Math.PI * radius;
  return { c, radius, circumference, arc: circumference * RING_SWEEP };
}
// SVG's <circle> starts at 3 o'clock and runs clockwise; dash offsets count from there.
const circle = (c: number, r: number) => `M${c + r} ${c}a${r} ${r} 0 1 1 ${-2 * r} 0a${r} ${r} 0 1 1 ${2 * r} 0`;

function Gauge270({ children }: { children: JSX.Element[] }) {
  const { c, radius, circumference, arc } = ringGeometry(8);
  const ticks: VectorPath[] = Array.from({ length: 19 }, (_, index) => ({
    d: `M${c} ${c - (radius - 18)}L${c} ${c - (radius - 12)}`, stroke: 'line-strong/65', strokeWidth: 1.5, strokeLinecap: 'round',
    transform: `rotate(${-RING_ROTATION + (index * 270) / 18} ${c} ${c})`,
  }));
  return (
    <view className="relative mx-auto aspect-square w-full max-w-[15rem]">
      <vector aria-hidden className="size-full" viewBox={[0, 0, RING_SIZE, RING_SIZE]} paths={[...ticks, {
        d: circle(c, radius), fill: 'none', stroke: 'surface-sunken', strokeWidth: 8, strokeDasharray: [arc, circumference - arc], strokeLinecap: 'round',
        transform: `rotate(${RING_ROTATION} ${c} ${c})`,
      }]} />
      <view className="absolute inset-0 grid place-items-center pb-2">{children}</view>
    </view>
  );
}

function StartDial({ onStart }: { onStart: () => void }) {
  return (
    <button onClick={onStart} aria-label={t('start')} data-testid="speedtest-start"
      className="speedtest-start focus-ring group relative mx-auto grid aspect-square w-full max-w-[15rem] place-items-center rounded-full">
      <view className="speedtest-halo absolute inset-8 rounded-full" aria-hidden />
      <Gauge270>
        <view className="speedtest-start-ring absolute inset-3 rounded-full" aria-hidden />
        <view className="grid justify-items-center gap-2">
          <view className="speedtest-start-button brand-gradient grid size-20 place-items-center rounded-full text-brand-foreground transition-transform duration-[var(--duration-fast)] group-active:scale-95 motion-reduce:transition-none">
            <Icon name="Gauge" size={30} />
          </view>
          <text className="text-item-title font-semibold">{t('startShort')}</text>
        </view>
      </Gauge270>
    </button>
  );
}

function LastTestRow({ previous }: { previous: Data['previous'] }) {
  if (!previous) return null;
  return (
    <row className="mt-4 items-center justify-between gap-3 border-t border-separator/60 pt-3" data-testid="speedtest-last">
      <column className="min-w-0">
        <text className="block text-supporting text-foreground">{t('lastTest')}</text>
        <text className="block text-caption text-muted-foreground">{previous.ago}</text>
      </column>
      <row className="shrink-0 items-center gap-3" dir="ltr">
        <row className="numeric items-center gap-1 text-item-title">
          <Icon name="ArrowDown" size={14} className="text-primary" />
          <text className="sr-only">{t('download')}</text>
          <text className="numeric text-item-title">{String(Math.round(previous.downMbps))}</text>
        </row>
        {previous.upMbps !== null ? (
          <row className="numeric items-center gap-1 text-item-title">
            <Icon name="ArrowUp" size={14} className="text-upload" />
            <text className="sr-only">{t('upload')}</text>
            <text className="numeric text-item-title">{String(Math.round(previous.upMbps))}</text>
          </row>
        ) : null}
        <text className="text-caption text-muted-foreground">Mbps</text>
      </row>
    </row>
  );
}

function AtRouterNote() {
  return (
    <row className="mt-3 items-start gap-3 rounded-lg bg-secondary px-4 py-3">
      <view className="mt-0.5 grid size-8 shrink-0 place-items-center rounded-full bg-accent-soft text-accent-foreground">
        <Icon name="Router" size={16} />
      </view>
      <column className="min-w-0 shrink">
        <text className="block text-item-title">{t('atRouterTitle')}</text>
        <text className="mt-0.5 block text-supporting text-muted-foreground">{t('atRouterBody')}</text>
      </column>
    </row>
  );
}

function MeasuresList() {
  const rows: { key: 'ping' | 'download' | 'upload'; icon: IconName; tint: string; hint: Key }[] = [
    { key: 'ping', icon: 'Timer', tint: 'bg-accent-soft text-accent-foreground', hint: 'pingHint' },
    { key: 'download', icon: 'ArrowDown', tint: 'bg-accent-soft text-accent-foreground', hint: 'downloadHint' },
    { key: 'upload', icon: 'ArrowUp', tint: 'bg-secondary text-upload', hint: 'uploadHint' },
  ];
  return (
    <column className={`mt-2 overflow-hidden ${CARD}`} role="list">
      {rows.map(({ key, icon, tint, hint }) => (
        <row key={key} role="listitem" className="items-center gap-3 border-t border-separator/60 px-4 py-3 first:border-t-0">
          <view className={`grid size-9 shrink-0 place-items-center rounded-md ${tint}`}><Icon name={icon} size={17} /></view>
          <column className="min-w-0 flex-1">
            <text className="text-item-title">{t(key)}</text>
            <text className="text-supporting text-muted-foreground">{t(hint)}</text>
          </column>
        </row>
      ))}
    </column>
  );
}

function StartScreen({ data, actions }: { data: Data; actions: Actions }) {
  return (
    <Shell>
      <HeroCard aside={<RouteLabel />}>
        <view className="mt-1"><StartDial onStart={() => actions.start()} /></view>
        <text className="mx-auto -mt-1 max-w-[16rem] text-center text-supporting text-muted-foreground">{t('durationHint')}</text>
        <LastTestRow previous={data.previous} />
      </HeroCard>
      <AtRouterNote />
      <column className="mt-section">
        <text role="heading" headingLevel={2} className={`px-1 ${EYEBROW}`}>{t('measuresTitle')}</text>
        <MeasuresList />
      </column>
    </Shell>
  );
}

const STEPS = ['ping', 'download', 'upload'] as const;
type StepState = 'past' | 'current' | 'future';
const PHASE_LABEL: Record<Phase, Key> = { preparing: 'preparing', ping: 'pingPhase', download: 'downloadPhase', upload: 'uploadPhase' };
const PHASE_ICON: Record<Phase, IconName> = { preparing: 'Gauge', ping: 'Timer', download: 'ArrowDown', upload: 'ArrowUp' };
const stepIndex = (phase?: Phase) => phase ? STEPS.indexOf(phase as (typeof STEPS)[number]) : -1;
function stepState(phase: Phase | undefined, index: number): StepState {
  const current = stepIndex(phase);
  if (current < 0 || index > current) return 'future';
  return index < current ? 'past' : 'current';
}
const formatClock = (seconds: number) => `${Math.floor(seconds / 60)}:${String(seconds % 60).padStart(2, '0')}`;

function StepRing({ phase, elapsed }: { phase?: Phase; elapsed: number }) {
  const stroke = 10;
  const { c, radius, circumference, arc } = ringGeometry(stroke);
  const third = arc / STEPS.length, segment = third - (stroke + 8), step = stepIndex(phase);
  const paths: VectorPath[] = STEPS.flatMap((key, index) => {
    const state = stepState(phase, index);
    const common = { d: circle(c, radius), fill: 'none', strokeWidth: stroke, strokeLinecap: 'round' as const,
      strokeDasharray: [segment, circumference - segment], strokeDashoffset: -(index * third + (stroke + 8) / 2), transform: `rotate(${RING_ROTATION} ${c} ${c})` };
    return [{ ...common, stroke: 'surface-sunken' }, ...(state === 'past' ? [{ ...common, stroke: 'url(#ring)' }] : []), ...(state === 'current' ? [{ ...common, stroke: 'accent' }] : [])];
  });
  return (
    <view className="relative mx-auto aspect-square w-full max-w-[15rem]" data-testid="speedtest-dial">
      <vector aria-hidden className="size-full rtl:-scale-x-100" viewBox={[0, 0, RING_SIZE, RING_SIZE]} paths={paths}
        gradients={{ ring: { type: 'linear', x1: RING_SIZE, y1: RING_SIZE, x2: 0, y2: 0, units: 'userSpaceOnUse', stops: [{ offset: 0, color: 'chart-1' }, { offset: 1, color: 'accent' }] } }} />
      <view className="absolute inset-0 grid place-items-center">
        <view className="relative grid size-44 place-items-center rounded-full">
          <vector aria-hidden className="absolute inset-0 animate-[speedtest-spin_1.6s_linear_infinite]" viewBox={[0, 0, 176, 176]}
            paths={[{ d: circle(88, 86.5), fill: 'none', stroke: 'url(#orbit)', strokeWidth: 3 }]}
            gradients={{ orbit: { type: 'conic', from: 0, stops: [{ offset: 0, color: '#00000000' }, { offset: 0.6, color: '#00000000' }, { offset: 1, color: 'chart-1' }] } }} />
          <view className="grid max-w-[9.5rem] justify-items-center gap-2 text-center">
            <view key={phase ?? 'none'} className="speedtest-pop brand-gradient grid size-11 place-items-center rounded-full text-brand-foreground">
              <Icon name={phase ? PHASE_ICON[phase] : 'Gauge'} size={20} />
            </view>
            <text className="text-center text-item-title font-semibold leading-snug" aria-live="polite" data-testid="speedtest-phase-label">
              {phase ? t(PHASE_LABEL[phase]) : t('testing')}
            </text>
            <row className="items-center gap-1.5">
              {step >= 0 ? (
                <>
                  <text className="text-caption text-muted-foreground">{t('stepOf', { n: step + 1 })}</text>
                  <text aria-hidden className="text-caption text-muted-foreground">·</text>
                </>
              ) : null}
              <text className="numeric text-caption text-muted-foreground" dir="ltr" aria-label={`${t('elapsed')} ${formatClock(elapsed)}`}>{formatClock(elapsed)}</text>
            </row>
          </view>
        </view>
      </view>
    </view>
  );
}

function StepMark({ state }: { state: StepState }) {
  if (state === 'past') return <Icon name="CircleCheck" size={13} className="text-success-subtle-foreground" />;
  if (state === 'current') return <view aria-hidden className="speedtest-live-dot size-2 shrink-0 rounded-full bg-primary" />;
  return <view aria-hidden className="size-2 shrink-0 rounded-full border border-separator" />;
}

function StepReadings({ reading, phase }: { reading: Reading | null; phase?: Phase }) {
  const figure = (key: (typeof STEPS)[number]) => {
    const value = key === 'ping' ? reading?.latencyMs : key === 'download' ? reading?.downMbps : reading?.upMbps;
    return value == null ? null : { value: String(value), unit: key === 'ping' ? 'ms' : 'Mbps' };
  };
  return (
    <view className="-mt-4 border-t border-separator/60 pt-3" data-testid="speedtest-phases">
      <grid className="grid grid-cols-3 gap-1.5" data-testid="speedtest-live-readings">
        {STEPS.map((key, index) => {
          const state = stepState(phase, index);
          const shown = figure(key);
          return (
            <view key={key} data-state={state} className="grid min-w-0 justify-items-center gap-1 rounded-md px-1 py-2 text-center data-[state=current]:bg-accent-soft/50">
              <row className={`max-w-full items-center gap-1.5`}>
                <StepMark state={state} />
                <text className={`truncate text-caption ${state === 'future' ? 'text-muted-foreground' : 'font-medium text-foreground'}`}>{t(key)}</text>
              </row>
              <row dir="ltr" className="numeric min-h-7 items-center">
                {shown ? (
                  <text className="speedtest-pop numeric text-section-title leading-tight">{shown.value}<span className="text-caption font-normal text-muted-foreground">{` ${shown.unit}`}</span></text>
                ) : state === 'current' ? (
                  <row className="items-center gap-1" aria-label={t('inProgress')}>
                    {['animate-[speedtest-dot_1.2s_ease-in-out_infinite]', 'animate-[speedtest-dot_1.2s_ease-in-out_0.15s_infinite]', 'animate-[speedtest-dot_1.2s_ease-in-out_0.3s_infinite]'].map(dot => (
                      <view key={dot} className={`block h-[5px] w-[5px] rounded-full bg-muted-foreground ${dot}`} />
                    ))}
                  </row>
                ) : state === 'past' ? (
                  <text className="text-supporting font-medium text-muted-foreground" data-testid="speedtest-step-done">{t('stepDone')}</text>
                ) : (
                  <text className="text-section-title text-muted-foreground">—</text>
                )}
              </row>
            </view>
          );
        })}
      </grid>
    </view>
  );
}

function RunningScreen({ data, actions }: { data: Data; actions: Actions }) {
  const phase = data.reading?.phase;
  const slow = data.elapsed >= 60;
  return (
    <Shell>
      <HeroCard aside={<RouteLabel />}>
        <view className="mt-2"><StepRing phase={phase} elapsed={data.elapsed} /></view>
        <StepReadings reading={data.reading} phase={phase} />
      </HeroCard>
      <row className="mt-3 items-start justify-center gap-1.5 px-3" aria-live="polite">
        <Icon name={slow ? 'Hourglass' : 'Info'} size={13} className="mt-0.5 text-muted-foreground" />
        <text className="min-w-0 shrink text-center text-caption text-muted-foreground">{slow ? t('slowHint') : t('keepOpen')}</text>
      </row>
      <spacer className="min-h-5 flex-1" />
      <Button size="lg" variant="secondary" className="mt-section w-full" onClick={() => actions.cancel()} data-testid="speedtest-cancel">
        <row className="items-center gap-2"><Icon name="X" size={16} /><text className="text-control">{t('cancel')}</text></row>
      </Button>
    </Shell>
  );
}

type Verdict = 'healthy' | 'ok' | 'issues' | 'partial';
function verdict({ latencyMs: latency, lossPct: loss }: Result): Verdict {
  if ((loss !== null && loss >= 2.5) || (latency !== null && latency >= 60)) return 'issues';
  if (latency === null || loss === null) return 'partial';
  return loss < 0.5 && latency < 30 ? 'healthy' : 'ok';
}
const VERDICT_BADGE = { healthy: 'success', ok: 'warning', issues: 'destructive', partial: 'secondary' } as const;

function Figure({ label, icon, tone, mbps }: { label: string; icon: IconName; tone: string; mbps: number | null }) {
  return (
    <view className="grid min-w-0 justify-items-center gap-1 px-2 text-center">
      <row className="items-center gap-1">
        <Icon name={icon} size={14} className={tone} />
        <text className="text-supporting text-muted-foreground">{label}</text>
      </row>
      {mbps === null ? (
        <view className="grid justify-items-center">
          <text className="speedtest-figure text-muted-foreground">—</text>
          <text className="text-caption font-normal text-muted-foreground" data-testid="speedtest-not-measured">{t('notMeasured')}</text>
        </view>
      ) : (
        <row className="items-baseline justify-center gap-1" dir="ltr">
          <text className="numeric speedtest-figure">{String(Math.round(mbps))}</text>
          <text className="text-supporting text-muted-foreground">Mbps</text>
        </row>
      )}
    </view>
  );
}

function UsageGrid({ result }: { result: Result }) {
  const { downMbps: down, upMbps: up, latencyMs: latency } = result;
  const rate = (measured: boolean, good: boolean) => (!measured ? 'unrated' : good ? 'ok' : 'weak') as 'ok' | 'weak' | 'unrated';
  const uses: { key: Key; rating: 'ok' | 'weak' | 'unrated' }[] = [
    { key: 'use4k', rating: rate(down !== null, (down ?? 0) >= 25) },
    { key: 'useCalls', rating: rate(down !== null && up !== null, (down ?? 0) >= 4 && (up ?? 0) >= 4) },
    { key: 'useGaming', rating: rate(down !== null && latency !== null, (down ?? 0) >= 15 && (latency ?? Infinity) < 60) },
    { key: 'useUploads', rating: rate(up !== null, (up ?? 0) >= 20) },
  ];
  const tone = {
    ok: { text: 'text-success-subtle-foreground', dot: 'bg-success-subtle-foreground', label: 'useOk' as Key },
    weak: { text: 'text-warning-subtle-foreground', dot: 'bg-warning-subtle-foreground', label: 'useWeak' as Key },
    unrated: { text: 'text-muted-foreground', dot: 'bg-muted-foreground', label: 'useUnrated' as Key },
  };
  return (
    <column>
      <text role="heading" headingLevel={2} className={`px-1 ${EYEBROW}`}>{t('handlesTitle')}</text>
      <grid className="mt-2 grid grid-cols-2 gap-2" role="list" data-testid="speedtest-uses">
        {uses.map(({ key, rating }) => (
          <view key={key} role="listitem" data-rating={rating} className={`grid min-w-0 content-start gap-1 px-3 py-2.5 ${CARD}`}>
            <text className="min-w-0 text-supporting font-medium text-foreground">{t(key)}</text>
            <row className="items-center gap-1.5">
              <view aria-hidden className={`size-1.5 shrink-0 rounded-full ${tone[rating].dot}`} />
              <text className={`text-caption ${tone[rating].text}`}>{t(tone[rating].label)}</text>
            </row>
          </view>
        ))}
      </grid>
    </column>
  );
}

function ResultScreen({ data, actions }: { data: Data; actions: Actions }) {
  const result = data.result!;
  const kind = verdict(result);
  return (
    <Shell>
      <view className="speedtest-rise">
        <HeroCard aside={<view className="mx-auto"><Badge variant={VERDICT_BADGE[kind]} className="shrink-0 gap-1.5">
          <Icon name={kind === 'healthy' ? 'CircleCheck' : 'Info'} size={13} /><text className="text-caption font-semibold">{t(kind)}</text>
        </Badge></view>}>
          <grid className="mt-4 grid grid-cols-2" data-testid="speedtest-result">
            <Figure label={t('download')} icon="ArrowDown" tone="text-primary" mbps={result.downMbps} />
            <Figure label={t('upload')} icon="ArrowUp" tone="text-upload" mbps={result.upMbps} />
          </grid>
        </HeroCard>
      </view>
      <view className="speedtest-rise mt-section"><UsageGrid result={result} /></view>
      <spacer className="min-h-5 flex-1" />
      <view className="speedtest-dock sticky bottom-0 -mx-gutter -mb-[max(1.5rem,env(safe-area-inset-bottom))] mt-section px-gutter pb-[max(1.5rem,env(safe-area-inset-bottom))] pt-4">
        <Button size="lg" className="w-full" onClick={() => actions.start()} data-testid="speedtest-run-again">
          <row className="items-center gap-2"><Icon name="ArrowUp" size={16} /><text className="text-control">{t('runAgain')}</text></row>
        </Button>
      </view>
    </Shell>
  );
}

export function render({ data, actions }: View<Actions, Data>) {
  if (data.requestStatus === 'ready') return <StartScreen data={data} actions={actions} />;
  if (data.requestStatus === 'success' && data.result) return <ResultScreen data={data} actions={actions} />;
  return <RunningScreen data={data} actions={actions} />;
}
