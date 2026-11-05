import { List, measureFrames, useLocal, type FrameStats, type View } from '@lingxia/view';
import '../../theme/index.css';
import { Button, Page, Section } from '../../shared/ui';
import type { PerfActions, PerfData, PerfScene } from '../../shared/perf-types';

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
const ms = (value: number | null | undefined) => (value == null ? '-' : value.toFixed(1));

interface Verdict { line: string; pass: boolean }

/** A scene passes when frames keep pace with the display, input shows within a few
 *  refreshes, or a returning page shows within a quarter second. */
function judge(spec: SceneSpec, stats: FrameStats): Verdict {
  const refresh = stats.refreshMs ?? 16.7;
  if (spec.judge === 'input') {
    const input = stats.inputToPresent.statsMs;
    const pass = input !== null && input.p95 <= refresh * 5;
    return { pass, line: `input→present p50 ${ms(input?.p50)} · p95 ${ms(input?.p95)} · p99 ${ms(input?.p99)} ms (${stats.inputToPresent.count})` };
  }
  if (spec.judge === 'reveal') {
    const reveal = stats.revealMs;
    const pass = reveal !== null && reveal.p95 <= 250;
    return { pass, line: `first present p50 ${ms(reveal?.p50)} · p95 ${ms(reveal?.p95)} · p99 ${ms(reveal?.p99)} ms (${stats.reveals} reveals)` };
  }
  const times = stats.frameTimeMs;
  const shown = stats.samples + stats.dropped;
  const pass = times !== null && times.p95 <= refresh * 2.5 && stats.dropped <= shown * 0.1;
  return { pass, line: `frame p50 ${ms(times?.p50)} · p95 ${ms(times?.p95)} · p99 ${ms(times?.p99)} ms` };
}

export function render({ data, actions }: View<PerfActions, PerfData>) {
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
      <row className="gap-2 flex-wrap">
        {order.map(key => (
          <Button key={key} testId={`perf-tab-${key}`} size="sm" variant={scene.value === key ? 'primary' : 'secondary'}
            disabled={running.value} onClick={() => { scene.set(key); result.set(''); verdict.set(''); }}>
            {scenes[key].label}
          </Button>
        ))}
      </row>
      <Section title={spec.label} description={`${spec.knob}: ${count}`} testId={`perf-scene-${scene.value}`}
        action={<Button testId="perf-run" size="sm" disabled={running.value} onClick={run}>{running.value ? 'Running…' : 'Run budget'}</Button>}>
        <row className="gap-2">
          {spec.counts.map(value => (
            <Button key={String(value)} testId={`perf-count-${value}`} size="sm" variant={value === count ? 'primary' : 'secondary'}
              disabled={running.value} onClick={() => counts.set({ ...counts.value, [scene.value]: value })}>
              {String(value)}
            </Button>
          ))}
        </row>
        <text data-testid="perf-result" data-verdict={verdict.value} className="text-xs font-mono tabular-nums">
          {result.value ? `${verdict.value.toUpperCase()} · ${result.value}` : running.value ? 'Measuring 5 s…' : 'Not run'}
        </text>
        <text data-testid="perf-tick" className="text-xs text-muted tabular-nums">{`Logic ticks seen: ${data.tick}`}</text>
        <Workload scene={scene.value} count={count} data={data} />
      </Section>
      <Section title="Stall" description="Runs View code past its time budget: the host replaces the page with recovery chrome." testId="perf-stall-section">
        <Button testId="perf-stall" variant="secondary" size="sm" onClick={stall}>Stall the View</Button>
      </Section>
    </Page>
  );
}

/** Busy past the View's execution budget, as a runaway handler would be. */
function stall() {
  const until = Date.now() + 2000;
  while (Date.now() < until) { /* spin */ }
}

function Workload({ scene, count, data }: { scene: PerfScene; count: number; data: PerfData }) {
  const active = data.running === scene;
  switch (scene) {
    case 'list': return <LongList count={count} step={active ? data.tick * 2 : -1} />;
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

/** Text, an image and a badge per row; a run moves two rows per tick. The list grows by
 *  pages at its end, so a large knob never asks one render for all its rows. */
function LongList({ count, step }: { count: number; step: number }) {
  // One cell per knob value: a new size starts again from one page.
  const shown = useLocal(`shown-${count}`, Math.min(count, LIST_PAGE));
  const rows = rowsOf(shown.value);
  const grow = () => { if (shown.value < count) shown.set(Math.min(count, shown.value + LIST_PAGE)); };
  return (
    <List data-testid="perf-list" data-shown={String(shown.value)} className="h-96 rounded-lg border border-line" items={rows} getKey={i => String(i)}
      onReachEnd={grow}
      anchor={step >= 0 ? { index: Math.min(step % count, shown.value - 1), align: 'start', seq: step } : undefined}
      renderItem={i => (
        <row data-testid={`perf-row-${i}`} className="items-center gap-3 px-3 py-2 border-b border-line">
          <image src={image(i)} alt="" className="w-10 h-10 rounded-full" />
          <column className="grow basis-0 min-w-0">
            <text className="text-sm font-medium truncate">Row {i + 1}</text>
            <text className="text-xs text-muted truncate">A second line of text to lay out in every row</text>
          </column>
          <text className="px-2 rounded-full bg-accent text-on-accent text-xs tabular-nums">{String(i % 100)}</text>
        </row>
      )} />
  );
}

/** Every node springs to the other side at once. */
function Springs({ count, phase }: { count: number; phase: boolean }) {
  return (
    <row data-testid="perf-springs" className="flex-wrap gap-1 h-80 overflow-hidden">
      {Array.from({ length: count }, (_, i) => (
        <view key={String(i)} className={`w-3 h-3 rounded-full bg-accent transition-transform duration-500 ease-spring-bouncy ${phase ? 'translate-y-8 scale-150' : 'translate-y-0 scale-100'}`} />
      ))}
    </row>
  );
}

/** Translucent layers over a spinning gradient: every frame blends the whole stack again. */
function Layers({ count, spinning }: { count: number; spinning: boolean }) {
  return (
    <stack data-testid="perf-layers" className="h-80 rounded-xl overflow-hidden bg-surface-sunken">
      <view className={`absolute -inset-16 bg-conic from-accent via-danger to-accent ${spinning ? 'animate-spin' : ''}`} />
      {Array.from({ length: count }, (_, i) => (
        <view key={String(i)}
          className={`absolute rounded-2xl shadow-xl ${i % 2 ? 'bg-linear-to-br from-accent/30 to-danger/30' : 'bg-surface/20'} ${i < 3 ? 'backdrop-blur-md' : ''}`}
          style={{ left: 8 + (i % 4) * 14, top: 8 + i * 12, width: 180, height: 120 }} />
      ))}
      <view className="absolute right-4 bottom-4 w-20 h-20 rounded-full bg-accent/50 blur-lg" />
    </stack>
  );
}

const PARAGRAPH = 'LingXia 灵匣 lays out mixed scripts: 文字排版需要处理中文、English words, emoji 🚀✨🎉 and numbers 12345 together. ';

/** A long mixed-script paragraph that grows while its box changes width. */
function Relayout({ count, tick }: { count: number; tick: number }) {
  const typed = 'typing… 輸入中 ⌨️ '.repeat(Math.floor(tick / 6) % 20);
  const width = 200 + ((tick * 3) % 160);
  return (
    <column data-testid="perf-text" className="h-80 overflow-hidden">
      <text className="text-sm" style={{ width }}>{PARAGRAPH.repeat(count) + typed}</text>
    </column>
  );
}

/** A box the finger drags; the HUD draws the finger's trail against the frames'. */
function Drag({ size }: { size: number }) {
  return (
    <column className="gap-2">
      <text className="text-xs text-muted">Drag the box across. With the HUD on (lxdev lxapp page hud on), red dots trail the finger and green dots the frames that showed it.</text>
      <row data-testid="perf-track" className="h-32 items-center rounded-xl bg-surface-sunken px-2">
        <view data-testid="perf-box" drag="x" dragSnapPoints={[0, 60, 120, 180, 240]} aria-label="Box"
          className="rounded-xl bg-accent" style={{ width: size, height: size }} />
      </row>
    </column>
  );
}

/** Rows of images at three decode sizes each; a run scrolls new ones in every tick. */
function ImageStream({ size, step }: { size: number; step: number }) {
  const rows = rowsOf(600);
  return (
    <List data-testid="perf-images" className="h-96 rounded-lg border border-line" items={rows} getKey={i => String(i)}
      anchor={step >= 0 ? { index: step % rows.length, align: 'start', seq: step } : undefined}
      renderItem={i => (
        <row className="items-end gap-2 px-3 py-2 border-b border-line">
          {[1, 0.75, 0.5].map((factor, k) => (
            <image key={String(k)} src={image(i * 3 + k)} alt="" className="rounded-md"
              style={{ width: Math.round(size * factor) + (i % 7), height: Math.round(size * factor) + (i % 7) }} />
          ))}
        </row>
      )} />
  );
}
