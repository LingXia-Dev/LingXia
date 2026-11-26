import { cx, useBox, useLocal, useView, type PointerEvent, type VectorPath, type Local } from '@lingxia/view';
import { Chart, ChartLegend, ChartReadout, type ChartLegendPosition, type ChartField, type ChartRecord, type ChartReference, type ChartRow, type ChartSeries, type ChartSize, type ChartSummarize } from '../Chart/Chart';
import { colorOf, keysOf, numberOf, numberIn, referenceEnds, referenceTop, roleOf, seriesFacts, seriesOf, textOf, type ChartColor } from '../internal/data';
import { leftPressed, pressed, pressedIndex, pressedKey, scrubbing } from '../internal/gesture';
import { box, indexAt, knockOuts, roundUp } from '../internal/geometry';
import { ReferenceLabels, ReferenceReading, referencePath, useReferences, zonePath } from '../internal/References';
import { Ticks } from '../internal/Ticks';
import { laidOutOnce, remember } from '../internal/memo';
import { barChartRecipe as r } from './BarChart.recipe';

export type BarChartLabels = 'fit' | 'all' | 'ends' | 'none';
export type BarChartValueLabels = 'auto' | 'always' | 'none';
export type BarChartLayout = 'grouped' | 'stacked';
export type BarChartOrientation = 'vertical' | 'horizontal';

/** `Row` is the app's own record type, so `x`, `y` and `keyField` name its fields and a typo fails to compile. */
export interface BarChartProps<Row extends ChartRecord<Row> = ChartRow> {
  /** The rows, one per category, in the order drawn. */
  rows: readonly Row[];
  /** The field that names each category: "month". */
  x: ChartField<Row>;
  /** The field that holds the number, or several series to draw side by side in each category. */
  y: ChartField<Row> | readonly ChartSeries<Row>[];
  /** The field that identifies a row across updates and in `value`. By default `x`. */
  keyField?: ChartField<Row>;
  /** The chart's accessible name: "Orders by weekday". */
  label: string;
  title?: string;
  description?: string;
  /** Turns a number into the text shown and read: units, separators, the locale. By default the number as the page's language writes it, grouped (`Intl.NumberFormat`). */
  formatValue?: (value: number) => string;
  /** Writes the summary a screen reader hears from the chart's facts; see `@liulidesign/charts/summary/en`. */
  summarize?: ChartSummarize;
  /**
   * Category labels under the bars: `fit` writes as many as fit side by side, each under its
   * bar; `all` every one, cut short where it is too long; `ends` the first and last. @default 'fit'
   */
  xLabels?: BarChartLabels;
  /**
   * How the chart shows its scale. `auto` writes each value over its bar up to eight bars of one
   * series, and otherwise the top of the scale, a round number, above the plot; `always` writes
   * every value of one series; `none` shows neither. @default 'auto'
   */
  valueLabels?: BarChartValueLabels;
  /** Shown when there is more than one series. @default 'bottom' */
  legendPosition?: ChartLegendPosition;
  /** The legend's accessible name; by default the chart's `label`. */
  legendLabel?: string;
  /** The chosen row's key; its bars keep their colour and the rest step back. */
  value?: string | null;
  /**
   * Makes each category a button, up to 36 of them; runs with its key, or `null` when the chosen
   * one is pressed again. A drag across the bars chooses each it passes.
   */
  onChange?: (key: string | null) => void;
  /** Where the choice starts when the chart keeps it itself: give this instead of `value`; `onChange` still hears each choice. */
  defaultValue?: string | null;
  /** A View-local cell holding the chosen key, read and written instead of `value` with `onChange`; `onChange` still hears each choice. */
  bind?: Local<string | null>;
  /** `stacked` piles a category's series into one bar, each on the one before, so the bar is their total; the scale reaches the largest. @default 'grouped' */
  layout?: BarChartLayout;
  /**
   * `horizontal` lays each category on its own line, its name and value over a bar that runs from
   * the start: for long names and ranked lists. It draws no scale, axis labels or references. @default 'vertical'
   */
  orientation?: BarChartOrientation;
  /** Series fields not drawn at present. @default [] */
  hiddenSeries?: readonly string[];
  /** Makes each legend entry a button that shows or hides its series. */
  onHiddenSeriesChange?: (fields: readonly string[]) => void;
  /** The series hidden at first when the chart keeps them itself: give this instead of `hiddenSeries`; the legend entries then toggle them, and `onHiddenSeriesChange` still hears each change. */
  defaultHiddenSeries?: readonly string[];
  /**
   * Levels drawn across the plot as dashed lines, each named at the end edge with its value: a target,
   * a limit. The range widens to hold them. @default []
   */
  references?: readonly ChartReference[];
  /** Widens the value range downward; zero is always in range. */
  domainMin?: number;
  /** Widens the value range upward, so charts that are compared share a scale. */
  domainMax?: number;
  /** @default 'md' */
  size?: ChartSize;
  /** @default false */
  isLoading?: boolean;
  error?: string;
  /** What to say when no row has a number. Defaults to "No data" in the page's language. */
  emptyLabel?: string;
  /** A second, quieter line under `emptyLabel`: what to do next. */
  emptyHint?: string;
  /** With `error`: tries again. */
  onRetry?: () => void;
  /** The words on the retry button. Defaults to "Try again" in the page's language. */
  retryLabel?: string;
  className?: string;
  /** On the chart; a bar's button is `<id>-<key>`. */
  'data-testid'?: string;
}

// Past this many categories a bar is too narrow to press; selection then comes from outside.
const maxPressable = 36;
const maxLabelled = 7;
const denseFrom = 16;
const maxValued = 8;
// The least space between two figures, and the figures' line before one is measured.
const figureGap = 4;
const figureLine = 20;
// A line's gap is this much wider than the figure on each side.
const knockOutPad = 2;
// The part of a category's width its bars' outlines take, and the gap between two bars of a group,
// as parts of the slot. The stroke that rounds a bar adds its width on each side.
const share = { solo: 0.5, group: 0.62, gap: 0.1 };
// The corner radius in points; narrow bars get a smaller one.
const radius = { regular: 3, dense: 1 };

interface BarPaths { full: VectorPath[]; dimmed: VectorPath[] }

// The clear space between two pieces of a stacked bar, in points.
const seamGap = 2;
/** A category's stacked reach: the sum of its positive values above zero and of its negative ones below. */
function piled(row: ChartRow, series: readonly { field: string }[]): [number, number] {
  let [up, down] = [0, 0];
  for (const each of series) {
    const value = numberOf(row, each.field);
    if (!Number.isFinite(value)) continue;
    if (value > 0) up += value;
    else down += value;
  }
  return [up, down];
}

const at = (value: number) => String(Math.round(value * 100) / 100);

// A horizontal bar's thickness in points, and the gap between the bars of a group.
const thickness = { xs: 6, sm: 8, md: 12, lg: 14 } satisfies Record<ChartSize, number>;
const groupGap = 2;

/**
 * A bar lying from `from` to `to` in points, rounded at its free end only; the base stays square, on
 * zero or against the piece before it. Every bar has the same commands, so a change of value or of
 * side morphs under `transition-[d]`.
 */
function lying(from: number, to: number, top: number, bottom: number, corner: number): string {
  const sign = to >= from ? 1 : -1;
  const c = Math.min(corner, Math.abs(to - from), (bottom - top) / 2);
  const sweep = sign > 0 ? 1 : 0;
  return `M${at(from)} ${at(top)}H${at(to - sign * c)}A${at(c)} ${at(c)} 0 0 ${sweep} ${at(to)} ${at(top + c)}V${at(bottom - c)}A${at(c)} ${at(c)} 0 0 ${sweep} ${at(to - sign * c)} ${at(bottom)}H${at(from)}Z`;
}

/**
 * One category's bars in points along its line, from zero: grouped, each series takes its own band
 * down the line; stacked, the pieces follow one another, a seam clear between them, and only the end
 * of the pile is rounded. Chosen elsewhere, the bars step back to a faint fill.
 */
function lyingPaths(
  row: ChartRow, series: readonly { field: string; color: ChartColor }[], high: number, depth: number, corner: number, grown: boolean,
  isStacked: boolean, barTall: number, wide: number, isDimmed: boolean,
): VectorPath[] {
  const span = high + depth > 0 ? high + depth : 1;
  const along = (value: number) => ((depth + value) / span) * wide;
  const paths: VectorPath[] = [];
  const add = (field: string, color: ChartColor, d: string) => paths.push({ key: field, d, fill: `${roleOf(color)}/${isDimmed ? 30 : 85}` });
  if (isStacked) {
    const tops = piled(row, series);
    const bases = [0, 0];
    for (const each of series) {
      const value = numberOf(row, each.field);
      if (!Number.isFinite(value) || value === 0) continue;
      const side = value > 0 ? 0 : 1;
      const from = bases[side];
      const to = from + value;
      bases[side] = to;
      const [a, b] = grown ? [from, to] : [0, 0];
      const sign = value > 0 ? 1 : -1;
      const isEnd = to === tops[side];
      let [x0, x1] = [along(a) + (a === 0 ? 0 : sign * seamGap / 2), along(b) - (isEnd ? 0 : sign * seamGap / 2)];
      if (sign * (x1 - x0) < 0) x0 = x1 = (x0 + x1) / 2;
      add(each.field, each.color, lying(x0, x1, 0, barTall, isEnd ? corner : 0));
    }
    return paths;
  }
  series.forEach((each, index) => {
    const value = numberOf(row, each.field);
    if (!Number.isFinite(value) || value === 0) return;
    const top = index * (barTall + groupGap);
    add(each.field, each.color, lying(along(0), along(grown ? value : 0), top, top + barTall, corner));
  });
  return paths;
}

/**
 * Every bar of a series as one outline in one box that runs from the top of the scale down to its
 * foot, so the zero line is a point in the outlines and moves with them. Each bar is left open
 * at zero: the fill closes it along the line, and a round-joined stroke of twice the radius, drawn
 * at a constant width however the box stretches, rounds the free end and stops flat at zero.
 */
function barPaths(
  rows: readonly ChartRow[], series: readonly { field: string; color: ChartColor }[], high: number, depth: number, chosen: number, corner: number, grown: boolean,
  stack?: { pointsTall: number },
): BarPaths {
  const out: BarPaths = { full: [], dimmed: [] };
  const slot = box / rows.length;
  if (stack) return stackedPaths(rows, series, high, depth, chosen, corner, grown, slot, stack.pointsTall);
  const isSolo = series.length === 1;
  const inner = slot * (isSolo ? share.solo : share.group);
  // The gaps take at most a third of the group, so many series still leave every bar a width.
  const gap = isSolo ? 0 : Math.min(slot * share.gap, inner / 3 / (series.length - 1));
  const width = (inner - (series.length - 1) * gap) / series.length;
  const span = high + depth > 0 ? high + depth : 1;
  const level = (value: number) => at(((high - value) / span) * box);
  series.forEach((each, column) => {
    const d = { full: '', dimmed: '' };
    rows.forEach((row, index) => {
      const value = numberOf(row, each.field);
      if (!Number.isFinite(value) || value === 0) return;
      // Before the first layout every bar stands at zero, so the bars grow to their values.
      const reach = grown ? value : 0;
      const x0 = slot * index + (slot - inner) / 2 + column * (width + gap);
      d[chosen >= 0 && chosen !== index ? 'dimmed' : 'full'] += `M${at(x0)} ${level(0)}V${level(reach)}h${at(width)}V${level(0)}`;
    });
    const role = roleOf(each.color);
    for (const side of ['full', 'dimmed'] as const)
      if (d[side]) out[side].push({ key: each.field, d: d[side], fill: role, stroke: role, strokeWidth: corner * 2, strokeLinejoin: 'round', vectorEffect: 'non-scaling-stroke' });
  });
  return out;
}

/**
 * One bar per category, its series piled from zero in order: positives up, negatives down. Like a
 * bar, a piece's outline is open along its base, so the base is flat and its far end is rounded by
 * the stroke, which reaches past it by the corner. A seam draws the far end back by the corner and
 * half the gap, and the next base back by half the gap, in plot units, leaving the gap clear.
 */
function stackedPaths(
  rows: readonly ChartRow[], series: readonly { field: string; color: ChartColor }[], high: number, depth: number, chosen: number, corner: number, grown: boolean, slot: number, pointsTall: number,
): BarPaths {
  const out: BarPaths = { full: [], dimmed: [] };
  const width = slot * share.solo;
  const span = high + depth > 0 ? high + depth : 1;
  const unit = box / span;
  const points = (value: number) => (pointsTall > 0 ? (value * box) / pointsTall : 0);
  const [baseInset, endInset] = [points(seamGap / 2), points(corner + seamGap / 2)];
  const level = (value: number) => (high - value) * unit;
  const bases = rows.map(() => [0, 0]);
  const tops = rows.map(row => piled(row, series));
  series.forEach(each => {
    const d = { full: '', dimmed: '' };
    rows.forEach((row, index) => {
      const value = numberOf(row, each.field);
      if (!Number.isFinite(value) || value === 0) return;
      const side = value > 0 ? 0 : 1;
      const from = bases[index][side];
      const to = from + value;
      bases[index][side] = to;
      const [a, b] = grown ? [from, to] : [0, 0];
      // Zero and the pile's far end keep their place; a seam with another piece is drawn back.
      const atZero = a === 0;
      const atEnd = to === tops[index][side];
      const sign = value > 0 ? 1 : -1;
      let [y0, y1] = [level(a) - (atZero ? 0 : sign * baseInset), level(b) + (atEnd ? 0 : sign * endInset)];
      if (sign * (y0 - y1) < 0) y0 = y1 = (y0 + y1) / 2;
      const x0 = slot * index + (slot - width) / 2;
      d[chosen >= 0 && chosen !== index ? 'dimmed' : 'full'] += `M${at(x0)} ${at(y0)}V${at(y1)}h${at(width)}V${at(y0)}`;
    });
    const role = roleOf(each.color);
    for (const side of ['full', 'dimmed'] as const)
      if (d[side]) out[side].push({ key: each.field, d: d[side], fill: role, stroke: role, strokeWidth: corner * 2, strokeLinejoin: 'round', vectorEffect: 'non-scaling-stroke' });
  });
  return out;
}

/** Amounts by category, as bars that stand on a zero line. */
export function BarChart<Row extends ChartRecord<Row>>({
  rows, x, y, keyField, label, title, description, formatValue: format, summarize, xLabels, valueLabels = 'auto', legendPosition = 'bottom', legendLabel,
  value, onChange, defaultValue, bind, layout = 'grouped', orientation = 'vertical', hiddenSeries: givenHidden, onHiddenSeriesChange: onHiddenChange, defaultHiddenSeries, references: given = [], domainMin, domainMax, size = 'md',
  isLoading = false, error, emptyLabel, emptyHint, onRetry, retryLabel, className, 'data-testid': testId,
}: BarChartProps<Row>) {
  // Without `value`, a `defaultValue` makes the choice the chart's own.
  const own = useLocal<string | null>('chosen', defaultValue ?? null);
  const isOwn = value === undefined && defaultValue !== undefined;
  const selectedKey = bind ? bind.value : isOwn ? own.value : value;
  const onSelect = bind ? (key: string | null) => { bind.set(key); onChange?.(key); } : isOwn ? (key: string | null) => { own.set(key); onChange?.(key); } : onChange;
  const ownHidden = useLocal<readonly string[]>('hidden', defaultHiddenSeries ?? []);
  const isOwnHidden = givenHidden === undefined && defaultHiddenSeries !== undefined;
  const hiddenSeries = isOwnHidden ? ownHidden.value : givenHidden ?? [];
  const onHiddenSeriesChange = isOwnHidden ? (fields: readonly string[]) => { ownHidden.set(fields); onHiddenChange?.(fields); } : onHiddenChange;
  // Only the fields named are read, and each of those holds a datum.
  const data = rows as readonly ChartRow[];
  // Without the app's own, values are written as the page's language writes numbers.
  const { env } = useView();
  const formatValue = format ?? numberIn(env.language);
  const references = given.filter(each => Number.isFinite(each.value));
  const all = seriesOf(y).map((series, index) => ({ ...series, color: colorOf(index, series.color) }));
  const shown = all.filter(series => !hiddenSeries.includes(series.field));
  const facts = shown.flatMap(series => seriesFacts(data, x, series, formatValue) ?? []);
  const isStacked = layout === 'stacked' && shown.length > 1;
  const values = isStacked ? data.flatMap(row => piled(row, shown)) : data.flatMap(row => shown.map(series => numberOf(row, series.field))).filter(Number.isFinite);
  const plot = useBox('plot');
  const widest = useBox('figure');
  const figureBoxes = data.map((_, index) => useBox(`figure-${index}`));
  // The smallest plot holds nothing but the marks.
  const isNone = valueLabels === 'none' || (valueLabels === 'auto' && size === 'xs');
  // Figures stand over the bars of one series only; a group has its readout instead.
  const isFigured = !isNone && shown.length === 1 && (valueLabels === 'always' || data.length <= maxValued);
  const longest = isFigured ? data.map(row => numberOf(row, shown[0].field)).filter(Number.isFinite).map(formatValue).reduce((a, b) => (b.length > a.length ? b : a), '') : '';
  const [plotWidth, figureWidth] = [plot.value?.width ?? 0, widest.value?.width ?? 0];
  // Figures that would touch, as at large text, give way to the scale at the ends unless asked for.
  const isCrowded = valueLabels === 'auto' && plotWidth > 0 && figureWidth + figureGap > plotWidth / Math.max(1, data.length);
  const scale = isNone ? 'none' : isFigured && !isCrowded ? 'each' : 'ends';
  // The figures' line is as tall as a figure, so a large one still sits on its bar.
  const lane = Math.max(figureLine, Math.ceil(widest.value?.height ?? 0));
  const levels = references.flatMap(referenceEnds);
  const [upTo, downTo] = [domainMax, domainMin].map(each => (each !== undefined && Number.isFinite(each) ? each : 0));
  const reach = [Math.max(0, upTo, ...values, ...levels), Math.max(0, -downTo, ...[...values, ...levels].map(value => -value))];
  // With the ends of the scale written out, the plot's edges are round numbers. All zeros still
  // stand on a scale of one, so zero is at the foot with everything that marks it.
  const [high, depth] = reach[0] + reach[1] > 0 ? (scale === 'ends' ? reach.map(roundUp) : reach) : [1, 0];
  const isSigned = depth > 0;
  // How far down the plot zero lies, as a percentage of its height.
  const zero = high + depth > 0 ? (high / (high + depth)) * 100 : 100;
  const isPressable = onSelect !== undefined && data.length <= maxPressable;
  const labels = xLabels ?? 'fit';
  const keys = keysOf(data, keyField ?? x);
  const chosen = selectedKey == null ? -1 : keys.indexOf(selectedKey);
  // Narrow bars take the small corner: many bars, counting each of a group, or the smallest plot, where a bar is a few points wide.
  const density = data.length * (isStacked ? 1 : Math.max(1, shown.length)) >= denseFrom || size === 'xs' ? 'dense' : 'regular';
  // Colours are role names, so the outlines outlive a theme change; they are rebuilt only for new data, encoding or choice.
  // Until the plot is first laid out the bars stand at zero; a chart that comes back does not grow them again.
  const grown = laidOutOnce(data, `bar:${label}`, plot.value !== null);
  const corner = radius[density];
  // The marks' height, which the plot units span: the plot less the corner kept at each free end.
  const pointsTall = Math.max(0, Math.round((plot.value?.height ?? 0) - corner - (isSigned ? corner : 0)));
  const paths = remember(data, 'bars', JSON.stringify([shown.map(series => [series.field, series.color]), high, depth, chosen, density, grown, isStacked && pointsTall]),
    () => barPaths(data, shown, high, depth, chosen, corner, grown, isStacked ? { pointsTall } : undefined));

  const reading = (row: ChartRow) => shown
    .filter(series => Number.isFinite(numberOf(row, series.field)))
    .map(series => (series.label ? `${series.label} ${formatValue(numberOf(row, series.field))}` : formatValue(numberOf(row, series.field))))
    .join(', ');

  const keyAt = (index: number) => keys[index];
  // With nothing chosen a plot too dense for buttons rests on its last category.
  const rest = chosen >= 0 ? chosen : data.length - 1;
  const scrub = onSelect && data.length > 0 ? (event: PointerEvent) => {
    const { phase } = event;
    const current = chosen >= 0 ? keyAt(chosen) : null;
    let index: number;
    if (phase === 'increment' || phase === 'decrement') index = Math.min(data.length - 1, Math.max(0, rest + (phase === 'increment' ? 1 : -1)));
    // The page took the press for a scroll: what it chose on the way is put back.
    else if (phase === 'cancel') return pressedKey() === current ? undefined : onSelect(pressedKey());
    else {
      index = indexAt(event.x, event.width, data.length, 'slots');
      if (phase === 'down') pressed(event, index, current);
      // A press that stays on its bar is the button's; one that lifts on a plot without buttons chooses there.
      if (phase === 'move' ? !scrubbing(event) || index === pressedIndex() : !(phase === 'up' && !isPressable)) return undefined;
      leftPressed();
    }
    return index === chosen ? undefined : onSelect(keyAt(index));
  } : undefined;
  // The plot scrubs only with a handler, so its axis travels with it.
  const plotPointer: { onPointer: (event: PointerEvent) => void; pointerAxis: 'x'; 'data-testid'?: string } | { onPointer?: undefined; pointerAxis?: undefined } =
    scrub ? { onPointer: scrub, pointerAxis: 'x', 'data-testid': testId === undefined ? undefined : `${testId}-plot` } : {};

  // One element per category: its name and values for assistive tech, and its press target.
  // With figures over the bars it also holds the figure, level with its bar's free end.
  const slots = data.map((row, index) => {
    const key = keys[index];
    const name = `${textOf(row, x)}: ${reading(row)}`;
    const value = numberOf(row, shown[0]?.field ?? '');
    // The figure's place: the bar's free end, as a share of a box as tall as the plot less the
    // figure's line. The box moves on the bars' timing, so the figure rises and moves with its bar.
    const end = grown ? value : 0;
    const down = high + depth > 0 ? ((high - end) / (high + depth)) * 100 : 0;
    const figure = scale === 'each' && shown.length === 1 && Number.isFinite(value) && [
      <view key="mover" className={cx(r.mover.base, r.motion.base)} style={{ translate: [0, `${down}%`] }}>
        <view aria-hidden className={r.valueSlot.base} style={{ height: lane }}><text box={figureBoxes[index]} className={r.value.base}>{formatValue(value)}</text></view>
      </view>,
      <view key="lane" className={r.lane.base} style={{ height: lane }} />,
    ];
    const slot = r.slot.base;
    const lanes = scale === 'each' && isSigned ? (value < 0 ? { paddingTop: lane } : { paddingBottom: lane }) : undefined;
    if (!isPressable) return <view key={key} role="listitem" aria-label={name} className={slot} style={lanes}>{figure}</view>;
    return (
      <button
        key={key}
        aria-label={name}
        aria-pressed={chosen === index}
        onClick={() => onSelect(chosen === index ? null : key)}
        data-testid={testId === undefined ? undefined : `${testId}-${key}`}
        className={cx(slot, r.slot.pressable)}
        style={lanes}
      >
        {figure}
      </button>
    );
  });

  const drawing = (list: VectorPath[], isDimmed: boolean) => list.length > 0 && (
    <vector paths={list} viewBox={[0, 0, box, box]} className={cx(r.drawing.base, r.motion.base, isDimmed ? r.drawing.dimmed : r.drawing.full)} />
  );
  // The marks are the scale's box: the corner's stroke stands past it at the bars' free ends, and the figures' lines lie outside it.
  const clear = scale === 'each' ? lane : 0;
  const markBox = { top: clear + corner, bottom: isSigned ? clear + corner : 0 };
  const span = high + depth > 0 ? high + depth : 1;
  const fall = (value: number) => (high - value) / span;
  const plotSize = plot.value;
  const down = (value: number) => markBox.top + fall(value) * ((plotSize?.height ?? 0) - markBox.top - markBox.bottom);
  // The figures over the bars are taken; a reference label goes round them.
  const figures = scale === 'each' && shown.length === 1 && plotSize ? data.flatMap((each, index) => {
    const value = numberOf(each, shown[0].field);
    if (!Number.isFinite(value)) return [];
    const [middle, y] = [(index + 0.5) * (plotSize.width / data.length), down(grown ? value : 0)];
    const [top, bottom] = value < 0 ? [y, y + lane + corner] : [y - lane - corner, y];
    return [{ start: middle - figureWidth / 2, end: middle + figureWidth / 2, top, bottom }];
  }) : [];
  const refs = useReferences(references, formatValue, down, plotSize, figures);
  // Each figure's own text box, where a reference line breaks so it never crosses digits.
  const slotWidth = (plotSize?.width ?? 0) / Math.max(1, data.length);
  const middles = data.map((_, index) => (index + 0.5) * slotWidth);
  const figureTexts = data.map((each, index) => {
    const value = numberOf(each, shown[0]?.field ?? '');
    const size = figureBoxes[index].value;
    if (figures.length === 0 || !Number.isFinite(value) || !size) return undefined;
    const y = down(grown ? value : 0);
    const top = (value < 0 ? y + corner : y - corner - lane) + (lane - size.height) / 2;
    return { start: middles[index] - size.width / 2, end: middles[index] + size.width / 2, top, bottom: top + size.height };
  });
  const toBox = (at: number) => (at / (plotSize?.width || 1)) * box;
  // One gap per bar, empty where no figure is crossed, so the line morphs with the bars.
  const level = (value: number) => Math.round(fall(value) * box * 100) / 100;
  const referenceLines = [
    ...references.flatMap((each, index) => (referenceEnds(each).length > 1 ? [zonePath(index, level(each.value), level(each.to!), 0, box)] : [])),
    ...references.flatMap((each, index) => referenceEnds(each).map((end, at) => referencePath(at ? index + references.length : index, level(end), 0, box,
      knockOuts(down(end), figureTexts, middles, knockOutPad).map(([start, end]) => [toBox(start), toBox(end)] as const)))),
  ];

  const row = (which: 'all' | 'ends', className: string) => (which === 'all' ? (
    <view className={cx(className, r.labels.all)}>
      {data.map((each, index) => <text key={keyAt(index)} className={cx(r.label.base, r.label.cell)}>{textOf(each, x)}</text>)}
    </view>
  ) : (
    <view className={cx(className, r.labels.ends)}>
      <text className={r.label.base}>{textOf(data[0], x)}</text>
      <text className={r.label.base}>{textOf(data[data.length - 1], x)}</text>
    </view>
  ));

  const picked = chosen >= 0 ? data[chosen] : undefined;
  const legend = all.length > 1 && (
    <ChartLegend
      label={legendLabel ?? label}
      items={all.map(series => ({ key: series.field, label: series.label, color: series.color, isOff: hiddenSeries.includes(series.field) }))}
      pressedState="shown"
      onItemPress={onHiddenSeriesChange && (field => onHiddenSeriesChange(hiddenSeries.includes(field) ? hiddenSeries.filter(each => each !== field) : [...hiddenSeries, field]))}
      data-testid={testId === undefined ? undefined : `${testId}-legend`}
    />
  );
  const frame = {
    label, title, description, size, isLoading, error, onRetry, retryLabel, emptyHint, emptyLabel, className, legendPosition,
    summary: summarize?.({ kind: 'bar', count: data.length, series: facts }),
    plotShape: 'bars' as const,
    isEmpty: !values.length,
    'data-testid': testId,
  };

  if (orientation === 'horizontal') {
    const barTall = thickness[size];
    const rowTall = isStacked ? barTall : shown.length * barTall + Math.max(0, shown.length - 1) * groupGap;
    const [rawHigh, rawDepth] = reach[0] + reach[1] > 0 ? reach : [1, 0];
    // The bars are drawn in points across the measured line; before that they wait at zero width.
    const wide = plot.value?.width ?? 0;
    const figureOf = (row: ChartRow) => {
      if (isNone || shown.length === 0) return undefined;
      if (shown.length === 1) return Number.isFinite(numberOf(row, shown[0].field)) ? formatValue(numberOf(row, shown[0].field)) : undefined;
      if (!isStacked) return undefined;
      const parts = shown.map(series => numberOf(row, series.field)).filter(Number.isFinite);
      return parts.length ? formatValue(parts.reduce((sum, each) => sum + each, 0)) : undefined;
    };
    const lines = data.map((row, index) => {
      const key = keys[index];
      const figure = figureOf(row);
      const isDimmed = chosen >= 0 && chosen !== index;
      const body = [
        <view key="head" aria-hidden className={r.lying.head}>
          <text className={cx(r.lying.name, chosen === index && r.lying.chosen)}>{textOf(row, x)}</text>
          {figure !== undefined && <text className={r.lying.figure}>{figure}</text>}
        </view>,
        <view key="track" aria-hidden className={r.lying.track} style={{ height: rowTall }}>
          {wide > 0 && (
            <vector
              paths={lyingPaths(row, shown, rawHigh, rawDepth, corner, grown, isStacked, barTall, wide, isDimmed)}
              viewBox={[0, 0, wide, rowTall]}
              className={cx(r.lying.drawing, r.motion.base)}
            />
          )}
          {rawDepth > 0 && <view className={r.lying.zero} style={{ insetInlineStart: `${(rawDepth / (rawHigh + rawDepth)) * 100}%` }} />}
        </view>,
      ];
      const name = `${textOf(row, x)}: ${reading(row)}`;
      if (!isPressable) return <view key={key} role="listitem" aria-label={name} className={r.lying.line}>{body}</view>;
      return (
        <button
          key={key}
          aria-label={name}
          aria-pressed={chosen === index}
          onClick={() => onSelect(chosen === index ? null : key)}
          data-testid={testId === undefined ? undefined : `${testId}-${key}`}
          className={cx(r.lying.line, r.lying.pressable)}
        >
          {body}
        </button>
      );
    });
    return (
      <Chart {...frame} size={isLoading || error !== undefined || !values.length ? size : 'fit'} legend={legend}>
        {/* The frame keeps drawn data left to right; lying bars are lines of text, so they follow the page. */}
        <view box={plot} dir={env.direction} role={isPressable ? undefined : 'list'} aria-label={isPressable ? undefined : label} className={r.lying.lines}>{lines}</view>
      </Chart>
    );
  }

  return (
    <Chart
      {...frame}
      // Bars that carry their values need no readout, and keep no line for one.
      readout={scale !== 'each' && (onSelect !== undefined || picked !== undefined) && (
        <ChartReadout
          label={picked && textOf(picked, x)}
          values={picked && shown.filter(series => Number.isFinite(numberOf(picked, series.field))).map(series => ({
            text: formatValue(numberOf(picked, series.field)), ...(shown.length > 1 ? { label: series.label, color: series.color } : {}),
          }))}
        />
      )}
      legend={legend}
      axis={labels === 'none' || !data.length ? undefined : labels === 'fit' ? (
        <view className={r.labels.fit}>
          <Ticks texts={data.map(row => textOf(row, x))} layout="slots">{row(data.length <= maxLabelled ? 'all' : 'ends', r.labels.cover)}</Ticks>
        </view>
      ) : row(labels === 'ends' && data.length < 2 ? 'all' : labels, r.labels.row)}
    >
      {scale === 'ends' && <text aria-hidden className={r.scale.base}>{formatValue(high)}</text>}
      <view box={plot} className={cx(r.plot.base, scale === 'ends' && r.plot.ruled, scale === 'ends' && isSigned && r.plot.floored)}>
        {isFigured && <text aria-hidden box={widest} className={cx(r.value.base, r.value.ghost)}>{longest}</text>}
        {/* Under everything the reference lines, then the marks: one drawing, a second, fainter one for bars that stepped back, and the zero line, moved on the bars' timing. */}
        <view aria-hidden className={r.marks.base} style={markBox}>
          {referenceLines.length > 0 && <vector paths={referenceLines} viewBox={[0, 0, box, box]} className={cx(r.drawing.base, r.motion.base)} />}
          {drawing(paths.full, false)}
          {drawing(paths.dimmed, true)}
          <view className={cx(r.zero.base, r.motion.base)} style={{ translate: [0, `${zero}%`] }}><view className={r.baseline.base} /></view>
          <ReferenceLabels texts={refs.texts} places={refs.places} shares={references.map(each => fall(referenceTop(each)) * 100)} />
        </view>
        {refs.ghost}
        <view
          role={isPressable ? undefined : 'list'}
          aria-label={isPressable ? undefined : label}
          className={r.slots.base}
          {...plotPointer}
          {...(scrub && !isPressable ? { 'aria-valuenow': rest, 'aria-valuemin': 0, 'aria-valuemax': data.length - 1, 'aria-valuetext': `${textOf(data[rest], x)}: ${reading(data[rest])}` } : {})}
        >
          {slots}
        </view>
        <ReferenceReading texts={refs.texts} />
      </view>
      {scale === 'ends' && isSigned && <text aria-hidden className={r.scale.base}>{formatValue(-depth)}</text>}
    </Chart>
  );
}
