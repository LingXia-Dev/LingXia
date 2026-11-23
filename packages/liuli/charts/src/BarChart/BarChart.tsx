import { cx, useBox, type PointerEvent, type VectorPath } from '@lingxia/view';
import { Chart, ChartLegend, ChartReadout, type ChartLegendPosition, type ChartRow, type ChartSeries, type ChartSize, type ChartSummarize } from '../Chart/Chart';
import { colorOf, keyOf, numberOf, plain, roleOf, seriesFacts, seriesOf, textOf, type ChartColor } from '../internal/data';
import { leftPressed, pressed, pressedIndex, pressedKey, scrubbing } from '../internal/gesture';
import { box, indexAt, roundUp, shares } from '../internal/geometry';
import { Ticks } from '../internal/Ticks';
import { laidOutOnce, remember } from '../internal/memo';
import { barChartRecipe as r } from './BarChart.recipe';

export type BarChartLabels = 'fit' | 'all' | 'ends' | 'none';
export type BarChartValueLabels = 'auto' | 'always' | 'none';

export interface BarChartProps {
  /** The rows, one per category, in the order drawn. */
  rows: readonly ChartRow[];
  /** The field that names each category: "month". */
  x: string;
  /** The field that holds the number, or several series to draw side by side in each category. */
  y: string | readonly ChartSeries[];
  /** The field that identifies a row across updates and in `value`. By default `x`. */
  keyField?: string;
  /** The chart's accessible name: "Orders by weekday". */
  label: string;
  title?: string;
  description?: string;
  /** Turns a number into the text shown and read: units, separators, the locale. By default the bare number. */
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
   * every value; `none` shows neither. @default 'auto'
   */
  valueLabels?: BarChartValueLabels;
  /** Shown when there is more than one series. @default 'bottom' */
  legend?: ChartLegendPosition;
  /** The legend's accessible name; by default the chart's `label`. */
  legendLabel?: string;
  /** The chosen row's key; its bars keep their colour and the rest step back. */
  value?: string | null;
  /**
   * Makes each category a button, up to 36 of them; runs with its key, or `null` when the chosen
   * one is pressed again. A drag across the bars chooses each it passes.
   */
  onChange?: (key: string | null) => void;
  /** Series fields not drawn at present. @default [] */
  hiddenSeries?: readonly string[];
  /** Makes each legend entry a button that shows or hides its series. */
  onHiddenSeriesChange?: (fields: readonly string[]) => void;
  /** Widens the value range downward; zero is always in range. */
  domainMin?: number;
  /** Widens the value range upward, so charts that are compared share a scale. */
  domainMax?: number;
  /** @default 'md' */
  size?: ChartSize;
  /** @default false */
  isLoading?: boolean;
  error?: string;
  /** What to say when no row has a number. */
  emptyLabel?: string;
  /** A second, quieter line under `emptyLabel`: what to do next. */
  emptyHint?: string;
  /** With `error`: tries again. Pass `retryLabel` with it. */
  onRetry?: () => void;
  /** The words on the retry button: "Try again". */
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
// The part of a category's width its bars' outlines take, and the gap between two bars of a group,
// as parts of the slot. The stroke that rounds a bar adds its width on each side.
const share = { solo: 0.5, group: 0.62, gap: 0.1 };
// The corner radius in points; narrow bars get a smaller one.
const radius = { regular: 3, dense: 1 };

interface BarPaths { up: VectorPath[]; upDimmed: VectorPath[]; down: VectorPath[]; downDimmed: VectorPath[] }

const at = (value: number) => String(Math.round(value * 100) / 100);

/**
 * Every bar of a series as one outline per side of zero: above it a bar runs from its value down
 * to the box's foot, below it from the box's top down to its value. A round-joined stroke of
 * twice the radius, drawn at a constant width however the box stretches, rounds the corners;
 * the plot hides the stroke at the zero line, so only the free end is round.
 */
function barPaths(
  rows: readonly ChartRow[], series: readonly { field: string; color: ChartColor }[], high: number, depth: number, chosen: number, corner: number, grown: boolean,
): BarPaths {
  const out: BarPaths = { up: [], upDimmed: [], down: [], downDimmed: [] };
  const slot = box / rows.length;
  const isSolo = series.length === 1;
  const inner = slot * (isSolo ? share.solo : share.group);
  const gap = isSolo ? 0 : slot * share.gap;
  const width = (inner - (series.length - 1) * gap) / series.length;
  series.forEach((each, column) => {
    const d = { up: '', upDimmed: '', down: '', downDimmed: '' };
    rows.forEach((row, index) => {
      const value = numberOf(row, each.field);
      if (!Number.isFinite(value) || value === 0) return;
      // Before the first layout every bar stands at zero, so the bars grow to their values.
      const reach = grown ? value : 0;
      const x0 = slot * index + (slot - inner) / 2 + column * (width + gap);
      const isDimmed = chosen >= 0 && chosen !== index;
      if (value > 0) d[isDimmed ? 'upDimmed' : 'up'] += `M${at(x0)} ${box}V${at((1 - reach / high) * box)}h${at(width)}V${box}Z`;
      else d[isDimmed ? 'downDimmed' : 'down'] += `M${at(x0)} 0V${at((-reach / depth) * box)}h${at(width)}V0Z`;
    });
    const role = roleOf(each.color);
    for (const side of ['up', 'upDimmed', 'down', 'downDimmed'] as const)
      if (d[side]) out[side].push({ key: each.field, d: d[side], fill: role, stroke: role, strokeWidth: corner * 2, strokeLinejoin: 'round', vectorEffect: 'non-scaling-stroke' });
  });
  return out;
}

/** Amounts by category, as bars that stand on a zero line. */
export function BarChart({
  rows: data, x, y, keyField, label, title, description, formatValue: format, summarize, xLabels, valueLabels = 'auto', legend = 'bottom', legendLabel,
  value: selectedKey, onChange: onSelect, hiddenSeries = [], onHiddenSeriesChange, domainMin, domainMax, size = 'md',
  isLoading = false, error, emptyLabel, emptyHint, onRetry, retryLabel, className, 'data-testid': testId,
}: BarChartProps) {
  const formatValue = format ?? plain;
  const all = seriesOf(y).map((series, index) => ({ ...series, color: colorOf(index, series.color) }));
  const shown = all.filter(series => !hiddenSeries.includes(series.field));
  const facts = shown.flatMap(series => seriesFacts(data, x, series, formatValue) ?? []);
  const values = data.flatMap(row => shown.map(series => numberOf(row, series.field))).filter(Number.isFinite);
  const plot = useBox('plot');
  const widest = useBox('figure');
  // The smallest plot holds nothing but the marks.
  const isNone = valueLabels === 'none' || (valueLabels === 'auto' && size === 'xs');
  const isFigured = !isNone && (valueLabels === 'always' || (data.length <= maxValued && shown.length === 1));
  const longest = isFigured ? data.map(row => formatValue(numberOf(row, shown[0]?.field ?? ''))).reduce((a, b) => (b.length > a.length ? b : a), '') : '';
  const [plotWidth, figureWidth] = [plot.value?.width ?? 0, widest.value?.width ?? 0];
  // Figures that would touch, as at large text, give way to the scale at the ends unless asked for.
  const isCrowded = valueLabels === 'auto' && plotWidth > 0 && figureWidth + figureGap > plotWidth / Math.max(1, data.length);
  const scale = isNone ? 'none' : isFigured && !isCrowded ? 'each' : 'ends';
  // The figures' line is as tall as a figure, so a large one still sits on its bar.
  const lane = Math.max(figureLine, Math.ceil(widest.value?.height ?? 0));
  const reach = [Math.max(0, domainMax ?? 0, ...values), Math.max(0, -(domainMin ?? 0), ...values.map(value => -value))];
  // With the ends of the scale written out, the plot's edges are round numbers.
  const [high, depth] = scale === 'ends' ? reach.map(roundUp) : reach;
  const isSigned = depth > 0;
  // The plot above zero and under it, as shares of its height.
  const [above, below] = shares(high > 0 ? high : 1, depth);
  const isPressable = onSelect !== undefined && data.length <= maxPressable;
  const labels = xLabels ?? 'fit';
  const chosen = data.findIndex((row, index) => keyOf(row, keyField ?? x, index) === selectedKey);
  // Narrow bars take the small corner: many bars, or the smallest plot, where a slot is a few points wide.
  const density = data.length >= denseFrom || size === 'xs' ? 'dense' : 'regular';
  // Colours are role names, so the outlines outlive a theme change; they are rebuilt only for new data, encoding or choice.
  // Until the plot is first laid out the bars stand at zero; a chart that comes back does not grow them again.
  const grown = laidOutOnce(data, `bar:${label}`, plot.value !== null);
  const paths = remember(data, 'bars', JSON.stringify([shown.map(series => [series.field, series.color]), high, depth, chosen, density, grown]),
    () => barPaths(data, shown, high, depth, chosen, radius[density], grown));

  const reading = (row: ChartRow) => shown
    .filter(series => Number.isFinite(numberOf(row, series.field)))
    .map(series => (series.label ? `${series.label} ${formatValue(numberOf(row, series.field))}` : formatValue(numberOf(row, series.field))))
    .join(', ');

  const keyAt = (index: number) => keyOf(data[index], keyField ?? x, index);
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

  // One element per category: its name and values for assistive tech, and its press target.
  // With figures over the bars it also holds the figure, level with its bar's free end.
  const slots = data.map((row, index) => {
    const key = keyOf(row, keyField ?? x, index);
    const name = `${textOf(row, x)}: ${reading(row)}`;
    const value = numberOf(row, shown[0]?.field ?? '');
    // The figure's place: the bar's free end, as a share of a box as tall as the plot less the
    // figure's line. The box moves on the bars' timing, so the figure rises and moves with its bar.
    const end = grown ? value : 0;
    const down = high + depth > 0 ? ((high - end) / (high + depth)) * 100 : 0;
    const figure = scale === 'each' && shown.length === 1 && Number.isFinite(value) && [
      <view key="mover" className={r.mover.base} style={{ translate: [0, `${down}%`] }}>
        <view aria-hidden className={r.valueSlot.base} style={{ height: lane }}><text className={r.value.base}>{formatValue(value)}</text></view>
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

  const drawing = (wrap: string, list: VectorPath[], isDimmed: boolean) => list.length > 0 && (
    <view className={cx(r.wrap.base, wrap)}>
      <vector paths={list} viewBox={[0, 0, box, box]} className={cx(r.drawing.base, isDimmed ? r.drawing.dimmed : r.drawing.full)} />
    </view>
  );
  const [wrapUp, wrapDown] = [r.wrap.up[density], r.wrap.down[density]];

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
  return (
    <Chart
      label={label}
      title={title}
      description={description}
      summary={summarize?.({ kind: 'bar', count: data.length, series: facts })}
      size={size}
      isLoading={isLoading}
      plotShape="bars"
      error={error}
      onRetry={onRetry}
      retryLabel={retryLabel}
      emptyHint={emptyHint}
      isEmpty={!values.length}
      emptyLabel={emptyLabel}
      className={className}
      data-testid={testId}
      // Bars that carry their values need no readout, and keep no line for one.
      readout={scale !== 'each' && (onSelect !== undefined || picked !== undefined) && (
        <ChartReadout
          label={picked && textOf(picked, x)}
          values={picked && shown.filter(series => Number.isFinite(numberOf(picked, series.field))).map(series => ({
            text: formatValue(numberOf(picked, series.field)), ...(shown.length > 1 ? { label: series.label, color: series.color } : {}),
          }))}
        />
      )}
      legendPosition={legend}
      legend={all.length > 1 && (
        <ChartLegend
          label={legendLabel ?? label}
          items={all.map(series => ({ key: series.field, label: series.label, color: series.color, isOff: hiddenSeries.includes(series.field) }))}
          pressedState="shown"
          onItemPress={onHiddenSeriesChange && (field => onHiddenSeriesChange(hiddenSeries.includes(field) ? hiddenSeries.filter(each => each !== field) : [...hiddenSeries, field]))}
          data-testid={testId === undefined ? undefined : `${testId}-legend`}
        />
      )}
      axis={labels === 'none' || !data.length ? undefined : labels === 'fit' ? (
        <view className={r.labels.fit}>
          <Ticks texts={data.map(row => textOf(row, x))} layout="slots">{row(data.length <= maxLabelled ? 'all' : 'ends', r.labels.cover)}</Ticks>
        </view>
      ) : row(labels, r.labels.row)}
    >
      {scale === 'ends' && <text aria-hidden className={r.scale.base}>{formatValue(high)}</text>}
      <view box={plot} className={cx(r.plot.base, scale === 'ends' && r.plot.ruled, scale === 'ends' && isSigned && r.plot.floored)}>
        {isFigured && <text aria-hidden box={widest} className={cx(r.value.base, r.value.ghost)}>{longest}</text>}
        {/* The marks: one drawing per side of zero, and a second, fainter one for bars that stepped back. */}
        <view aria-hidden className={r.marks.base} style={scale === 'each' ? { top: lane, ...(isSigned ? { bottom: lane } : {}) } : undefined}>
          <view className={r.region.base} style={{ flexGrow: above }}>
            {drawing(wrapUp, paths.up, false)}
            {drawing(wrapUp, paths.upDimmed, true)}
          </view>
          <view className={r.baseline.base} />
          {isSigned && (
            <view className={r.region.base} style={{ flexGrow: below }}>
              {drawing(wrapDown, paths.down, false)}
              {drawing(wrapDown, paths.downDimmed, true)}
            </view>
          )}
        </view>
        <view
          role={isPressable ? undefined : 'list'}
          aria-label={isPressable ? undefined : label}
          className={r.slots.base}
          {...(scrub ? { onPointer: scrub, pointerAxis: 'x' as const, 'data-testid': testId === undefined ? undefined : `${testId}-plot` } : {})}
          {...(scrub && !isPressable ? { 'aria-valuenow': rest, 'aria-valuemin': 0, 'aria-valuemax': data.length - 1, 'aria-valuetext': `${textOf(data[rest], x)}: ${reading(data[rest])}` } : {})}
        >
          {slots}
        </view>
      </view>
      {scale === 'ends' && isSigned && <text aria-hidden className={r.scale.base}>{formatValue(-depth)}</text>}
    </Chart>
  );
}
