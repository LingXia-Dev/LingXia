import { t, List, measureFrames, useLocal, type FrameStats, type MessageKey } from '@lingxia/view';
import '../../theme/index.css';
import { Button, Page, Section } from '../../shared/ui';
import { judgeFrames, judgeInput, judgeReveal, type Verdict } from '../../shared/perf-judge';
import type { View } from './$types';

type PerfScene = Exclude<View['data']['running'], ''>;
type PerfRow = View['data']['rows'][number];

// A standard workload per pipeline stage. "Run budget" drives one scene for 5 s and
// judges the frames the renderer measured; specs drive the same scenes with real gestures.
const RUN_MS = 5000;

interface SceneSpec {
  label: string;
  knob: string;
  counts: number[];
  /** What the budget judges. */
  judge: 'frames' | 'input' | 'reveal';
}

const scenes: Record<PerfScene, SceneSpec> = {
  list: { label: 'List', knob: 'rows', counts: [100, 1000, 5000, 20000], judge: 'frames' },
  data: { label: 'Data', knob: 'rows', counts: [100, 1000, 5000, 20000], judge: 'frames' },
  state: { label: 'State', knob: 'records', counts: [100, 1000, 5000], judge: 'frames' },
  animation: { label: 'Springs', knob: 'nodes', counts: [100, 250, 500], judge: 'frames' },
  fill: { label: 'Fill', knob: 'layers', counts: [4, 8, 16], judge: 'frames' },
  text: { label: 'Text', knob: 'paragraphs', counts: [1, 4, 12], judge: 'frames' },
  touch: { label: 'Touch', knob: 'box px', counts: [48, 96], judge: 'input' },
  transition: { label: 'Push/pop', knob: 'rows', counts: [20, 100, 400], judge: 'reveal' },
  images: { label: 'Images', knob: 'image px', counts: [48, 96, 160], judge: 'frames' },
};
const order = Object.keys(scenes) as PerfScene[];

// One frozen array per size: a render that passes the same array does no per-row work.
const rowSets = new Map<number, readonly number[]>();
function rowsOf(count: number): readonly number[] {
  let rows = rowSets.get(count);
  if (!rows) {
    rows = Object.freeze(Array.from({ length: count }, (_, i) => i));
    rowSets.set(count, rows);
  }
  return rows;
}

const image = (index: number) => `public/perf/img-${String(index % 16).padStart(2, '0')}.png`;

/** The shared bar: frames keep pace with the display, input shows within a few refreshes,
 *  a returning page shows within a quarter second. */
function judge(spec: SceneSpec, stats: FrameStats): Verdict {
  if (spec.judge === 'input') return judgeInput(stats);
  if (spec.judge === 'reveal') return judgeReveal(stats);
  return judgeFrames(stats);
}

export function render({ data, actions }: View) {
  const scene = useLocal<PerfScene>('scene', 'list');
  const counts = useLocal<Record<string, number>>('counts', {});
  const result = useLocal('result', '');
  const verdict = useLocal('verdict', '');
  const running = useLocal('running', false);
  const spec = scenes[scene.value];
  const count = counts.value[scene.value] ?? spec.counts[1] ?? spec.counts[0];
  const run = async () => {
    if (running.value) return;
    running.set(true);
    result.set('');
    verdict.set('');
    const measured = measureFrames(RUN_MS);
    actions.drive({ scene: scene.value, ms: RUN_MS, count }).catch(() => {});
    const stats = await measured;
    const { line, pass } = judge(spec, stats);
    result.set(`${line} · dropped ${stats.dropped} · jank ${stats.jank} · ${stats.frames} frames`);
    verdict.set(pass ? 'pass' : 'fail');
    running.set(false);
  };
  return (
    <Page testId="perf-lab" data={{ 'data-scene': scene.value, 'data-count': String(count), 'data-verdict': verdict.value }}>
      <view className="flex-row gap-2 flex-wrap">
        {order.map(key => (
          <Button key={key} testId={`perf-tab-${key}`} size="sm" variant={scene.value === key ? 'primary' : 'secondary'}
            disabled={running.value} onClick={() => { scene.set(key); result.set(''); verdict.set(''); }}>
            {t(`perfLab.scene.${key}`)}
          </Button>
        ))}
      </view>
      <Section title={t(`perfLab.scene.${scene.value}` as MessageKey)} description={`${t(`perfLab.knob.${spec.knob.replace(' px', 'Px')}` as MessageKey)}: ${count}`} testId={`perf-scene-${scene.value}`}
        action={<Button testId="perf-run" size="sm" disabled={running.value} onClick={run}>{running.value ? t('perfLab.running') : t('perfLab.runBudget')}</Button>}>
        <view className="flex-row gap-2">
          {spec.counts.map(value => (
            <Button key={String(value)} testId={`perf-count-${value}`} size="sm" variant={value === count ? 'primary' : 'secondary'}
              disabled={running.value} onClick={() => counts.set({ ...counts.value, [scene.value]: value })}>
              {String(value)}
            </Button>
          ))}
        </view>
        <text data-testid="perf-result" data-verdict={verdict.value} className="text-xs font-mono tabular-nums">
          {result.value ? `${verdict.value.toUpperCase()} · ${result.value}` : running.value ? t('perfLab.measuring') : t('perfLab.notRun')}
        </text>
        <text data-testid="perf-tick" className="text-xs text-muted tabular-nums">{t('perfLab.ticks', { n: data.tick })}</text>
        <Workload scene={scene.value} count={count} data={data} />
      </Section>
      <Section title={t('perfLab.stall')} description={t('perfLab.runsViewCodePast')} testId="perf-stall-section">
        <Button testId="perf-stall" variant="secondary" size="sm" onClick={stall}>{t('perfLab.stallTheView')}</Button>
      </Section>
    </Page>
  );
}

/** Busy past the View's execution budget, as a runaway handler would be. */
function stall() {
  // Past the interrupt limit of a debug build too (8x budget), so every profile fails the page.
  const until = Date.now() + 5000;
  while (Date.now() < until) { /* spin */ }
}

function Workload({ scene, count, data }: { scene: PerfScene; count: number; data: View['data'] }) {
  const active = data.running === scene;
  switch (scene) {
    case 'list': return <LongList count={count} step={-1} glide={active} />;
    case 'data': return <LongList count={count} step={active ? data.tick * 2 : -1} glide={false} />;
    case 'state': return <Records rows={active ? data.rows : []} count={count} />;
    case 'animation': return <Springs count={count} phase={active && data.phase} />;
    case 'fill': return <Layers count={count} spinning={active} />;
    case 'text': return <Relayout count={count} tick={active ? data.tick : 0} />;
    case 'touch': return <Drag size={count} />;
    case 'transition': return <text className="text-xs text-muted">Pushes Perf target with {count} rows and pops back for 5 s; judged on this page's reveals.</text>;
    case 'images': return <ImageStream size={count} step={active ? data.tick * 2 : -1} />;
  }
}

/** Rows are added this many at a time, as the list nears its end. */
const LIST_PAGE = 2000;

/** How far a List run scrolls; its glide lasts the whole run. */
const GLIDE_ROWS = 90;

/** Text, an image and a badge per row. A List run glides natively (the View runs only when the
 *  window shifts); a Data run moves Logic's anchor two rows per tick, a full render each. The
 *  list grows by pages at its end, so a large knob never asks one render for all its rows. */
function LongList({ count, step, glide }: { count: number; step: number; glide: boolean }) {
  // One cell per knob value: a new size starts again from one page.
  const shown = useLocal(`shown-${count}`, Math.min(count, LIST_PAGE));
  const rows = rowsOf(shown.value);
  const grow = () => { if (shown.value < count) shown.set(Math.min(count, shown.value + LIST_PAGE)); };
  return (
    <List data-testid="perf-list" data-shown={String(shown.value)} className="h-96 rounded-lg border border-line" items={rows} getKey={i => String(i)}
      onReachEnd={grow}
      anchor={glide ? { index: Math.min(GLIDE_ROWS, shown.value - 1), align: 'start', duration: RUN_MS }
        : step >= 0 ? { index: Math.min(step % count, shown.value - 1), align: 'start', seq: step } : undefined}
      renderItem={i => (
        <view data-testid={`perf-row-${i}`} className="flex-row items-center gap-3 px-3 py-2 border-b border-line">
          <image src={image(i)} alt="" className="w-10 h-10 rounded-full" />
          <view className="grow basis-0 min-w-0">
            <text className="text-sm font-medium truncate">{t('video.row', { id: i + 1 })}</text>
            <text className="text-xs text-muted truncate">{t('perfLab.aSecondLineOf')}</text>
          </view>
          <text className="px-2 rounded-full bg-accent text-on-accent text-xs tabular-nums">{String(i % 100)}</text>
        </view>
      )} />
  );
}

/** Logic owns the records; each tick changes one of them, so a frame's cost should follow the
 *  change, not the size of the page's data. */
function Records({ rows, count }: { rows: readonly PerfRow[]; count: number }) {
  if (rows.length === 0) return <text className="text-xs text-muted">{t('perfLab.stateHint', { count })}</text>;
  return (
    <List data-testid="perf-records" data-rows={String(rows.length)} className="h-96 rounded-lg border border-line" items={rows} getKey={row => String(row.id)}
      renderItem={row => (
        <view data-testid={`perf-record-${row.id}`} className="flex-row items-center gap-3 px-3 py-2 border-b border-line">
          <view className="grow basis-0 min-w-0">
            <text className="text-sm font-medium truncate">{row.title}</text>
            <text className="text-xs text-muted truncate">{row.note}</text>
          </view>
          <text className={`px-2 rounded-full text-xs tabular-nums ${row.done ? 'bg-accent text-on-accent' : 'bg-surface-sunken'}`}>{String(row.count)}</text>
        </view>
      )} />
  );
}

/** Every node springs to the other side at once. */
function Springs({ count, phase }: { count: number; phase: boolean }) {
  return (
    <view data-testid="perf-springs" className="flex-row flex-wrap gap-1 h-80 overflow-hidden">
      {Array.from({ length: count }, (_, i) => (
        <view key={String(i)} className={`w-3 h-3 rounded-full bg-accent transition-transform duration-500 ease-spring-bouncy ${phase ? 'translate-y-8 scale-150' : 'translate-y-0 scale-100'}`} />
      ))}
    </view>
  );
}

/** Translucent layers over a spinning gradient: every frame blends the whole stack again. */
function Layers({ count, spinning }: { count: number; spinning: boolean }) {
  return (
    <view data-testid="perf-layers" className="grid *:col-start-1 *:row-start-1 h-80 rounded-xl overflow-hidden bg-surface-sunken">
      <view className={`absolute -inset-16 bg-conic from-accent via-danger to-accent ${spinning ? 'animate-spin' : ''}`} />
      {Array.from({ length: count }, (_, i) => (
        <view key={String(i)}
          className={`absolute rounded-2xl shadow-xl ${i % 2 ? 'bg-linear-to-br from-accent/30 to-danger/30' : 'bg-surface/20'} ${i < 3 ? 'backdrop-blur-md' : ''}`}
          style={{ left: 8 + (i % 4) * 14, top: 8 + i * 12, width: 180, height: 120 }} />
      ))}
      <view className="absolute right-4 bottom-4 w-20 h-20 rounded-full bg-accent/50 blur-lg" />
    </view>
  );
}

const PARAGRAPH = 'LingXia 灵匣 lays out mixed scripts: 文字排版需要处理中文、English words, emoji 🚀✨🎉 and numbers 12345 together. ';

/** A long mixed-script paragraph that grows while its box changes width. */
function Relayout({ count, tick }: { count: number; tick: number }) {
  const typed = 'typing… 輸入中 ⌨️ '.repeat(Math.floor(tick / 6) % 20);
  const width = 200 + ((tick * 3) % 160);
  return (
    <view data-testid="perf-text" className="h-80 overflow-hidden">
      <text className="text-sm" style={{ width }}>{PARAGRAPH.repeat(count) + typed}</text>
    </view>
  );
}

/** A box the finger drags; the HUD draws the finger's trail against the frames'. */
function Drag({ size }: { size: number }) {
  return (
    <view className="gap-2">
      <text className="text-xs text-muted">{t('perfLab.dragTheBoxAcross')}</text>
      <view data-testid="perf-track" className="flex-row h-32 items-center rounded-xl bg-surface-sunken px-2">
        <view data-testid="perf-box" drag="x" dragSnapPoints={[0, 60, 120, 180, 240]} aria-label={t('perfLab.box')}
          className="rounded-xl bg-accent" style={{ width: size, height: size }} />
      </view>
    </view>
  );
}

/** Rows of images at three decode sizes each; a run scrolls new ones in every tick. */
function ImageStream({ size, step }: { size: number; step: number }) {
  const rows = rowsOf(600);
  return (
    <List data-testid="perf-images" className="h-96 rounded-lg border border-line" items={rows} getKey={i => String(i)}
      anchor={step >= 0 ? { index: step % rows.length, align: 'start', seq: step } : undefined}
      renderItem={i => (
        <view className="flex-row items-end gap-2 px-3 py-2 border-b border-line">
          {[1, 0.75, 0.5].map((factor, k) => (
            <image key={String(k)} src={image(i * 3 + k)} alt="" className="rounded-md"
              style={{ width: Math.round(size * factor) + (i % 7), height: Math.round(size * factor) + (i % 7) }} />
          ))}
        </view>
      )} />
  );
}
