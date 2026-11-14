import { cx } from '@lingxia/view';
import { Chart, ChartLegend, ChartReadout, type ChartLegendPosition, type ChartRow, type ChartSeries, type ChartSize, type ChartSummarize } from '../Chart/Chart';
import { colorOf, keyOf, numberOf, plain, seriesFacts, seriesOf, textOf } from '../internal/data';
import { roundUp } from '../internal/geometry';
import { barChartRecipe as r } from './BarChart.recipe';

export type BarChartLabels = 'all' | 'ends' | 'none';
export type BarChartValueLabels = 'auto' | 'always' | 'none';

export interface BarChartProps {
  /** The rows, one per category, in the order drawn. */
  data: readonly ChartRow[];
  /** The field that names each category: "month". */
  x: string;
  /** The field that holds the number, or several series to draw side by side in each category. */
  y: string | readonly ChartSeries[];
  /** The field that identifies a row across updates and in `selectedKey`. By default `x`. */
  keyField?: string;
  /** The chart's accessible name: "Orders by weekday". */
  label: string;
  title?: string;
  description?: string;
  /** Turns a number into the text shown and read: units, separators, the locale. By default the bare number. */
  formatValue?: (value: number) => string;
  /** Writes the summary a screen reader hears from the chart's facts; see `@liulidesign/charts/summary/en`. */
  summarize?: ChartSummarize;
  /** Category labels under the bars: every one, or the first and last. By default every one up to seven bars. */
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
  selectedKey?: string | null;
  /** Makes each category a button; runs with its key, or `null` when the chosen one is pressed again. Up to 36 categories. */
  onSelect?: (key: string | null) => void;
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
  className?: string;
  /** On the chart; a bar's button is `<id>-<key>`. */
  'data-testid'?: string;
}

// Past this many categories a bar is too narrow to press; selection then comes from outside.
const maxPressable = 36;
const maxLabelled = 7;
const denseFrom = 16;
const maxValued = 8;
// The part of a category's width its bars take; the rest is the gap to its neighbours.
const share = { regular: 0.68, dense: 0.8 };

/** Amounts by category, as bars that stand on a zero line. */
export function BarChart({
  data, x, y, keyField, label, title, description, formatValue: format, summarize, xLabels, valueLabels = 'auto', legend = 'bottom', legendLabel,
  selectedKey, onSelect, hiddenSeries = [], onHiddenSeriesChange, domainMin, domainMax, size = 'md',
  isLoading = false, error, emptyLabel, className, 'data-testid': testId,
}: BarChartProps) {
  const formatValue = format ?? plain;
  const all = seriesOf(y).map((series, index) => ({ ...series, color: colorOf(index, series.color) }));
  const shown = all.filter(series => !hiddenSeries.includes(series.field));
  const facts = shown.flatMap(series => seriesFacts(data, x, series, formatValue) ?? []);
  const values = data.flatMap(row => shown.map(series => numberOf(row, series.field))).filter(Number.isFinite);
  // The smallest plot holds nothing but the marks.
  const scale = valueLabels === 'none' || (valueLabels === 'auto' && size === 'xs') ? 'none'
    : valueLabels === 'always' || (data.length <= maxValued && shown.length === 1) ? 'each' : 'ends';
  const reach = [Math.max(0, domainMax ?? 0, ...values), Math.max(0, -(domainMin ?? 0), ...values.map(value => -value))];
  // With the ends of the scale written out, the plot's edges are round numbers.
  const [high, depth] = scale === 'ends' ? reach.map(roundUp) : reach;
  const isSigned = depth > 0;
  const isPressable = onSelect !== undefined && data.length <= maxPressable;
  const labels = xLabels ?? (data.length <= maxLabelled ? 'all' : 'ends');
  const chosen = data.findIndex((row, index) => keyOf(row, keyField ?? x, index) === selectedKey);
  const density = data.length >= denseFrom ? 'dense' : 'regular';
  const margin = Math.round(((shown.length * (1 - share[density])) / (2 * share[density])) * 1000) / 1000;

  const reading = (row: ChartRow) => shown
    .filter(series => Number.isFinite(numberOf(row, series.field)))
    .map(series => (series.label ? `${series.label} ${formatValue(numberOf(row, series.field))}` : formatValue(numberOf(row, series.field))))
    .join(', ');

  const slots = data.map((row, index) => {
    const key = keyOf(row, keyField ?? x, index);
    const isDimmed = chosen >= 0 && chosen !== index;
    const columns = shown.map(series => {
      const value = numberOf(row, series.field);
      const [above, below] = Number.isFinite(value) ? [Math.max(value, 0), Math.max(-value, 0)] : [0, 0];
      const paint = isDimmed ? r.bar.dimmed[series.color] : r.bar.color[series.color];
      const figure = Number.isFinite(value) && <text className={r.value.base}>{formatValue(value)}</text>;
      return (
        <view key={series.field} className={r.column.base}>
          <view className={r.space.base} style={{ flexGrow: high - above }} />
          {/* Every column keeps the figure's line, above and below, so all bars share one zero line. */}
          {scale === 'each' && <view aria-hidden className={r.valueSlot.base}>{below === 0 && figure}</view>}
          <view className={cx(r.bar.base, r.bar.up, paint)} style={{ flexGrow: above }} />
          <view className={cx(r.bar.base, r.bar.down, paint)} style={{ flexGrow: below }} />
          {scale === 'each' && isSigned && <view aria-hidden className={r.valueSlot.base}>{below > 0 && figure}</view>}
          <view className={r.space.base} style={{ flexGrow: depth - below }} />
        </view>
      );
    });
    const name = `${textOf(row, x)}: ${reading(row)}`;
    const content = [
      <view key="before" className={r.space.base} style={{ flexGrow: margin }} />,
      ...columns,
      <view key="after" className={r.space.base} style={{ flexGrow: margin }} />,
    ];
    if (!isPressable) return <view key={key} role="listitem" aria-label={name} className={r.slot.base}>{content}</view>;
    return (
      <button
        key={key}
        aria-label={name}
        aria-pressed={chosen === index}
        onClick={() => onSelect(chosen === index ? null : key)}
        data-testid={testId === undefined ? undefined : `${testId}-${key}`}
        className={cx(r.slot.base, r.slot.pressable)}
      >
        {content}
      </button>
    );
  });

  const picked = chosen >= 0 ? data[chosen] : undefined;
  return (
    <Chart
      label={label}
      title={title}
      description={description}
      summary={summarize?.({ kind: 'bar', count: data.length, series: facts })}
      size={size}
      isLoading={isLoading}
      error={error}
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
      axis={labels === 'none' || !data.length ? undefined : labels === 'all' ? (
        <view className={r.labels.all}>
          {data.map((row, index) => <text key={keyOf(row, keyField ?? x, index)} className={cx(r.label.base, r.label.cell)}>{textOf(row, x)}</text>)}
        </view>
      ) : (
        <view className={r.labels.ends}>
          <text className={r.label.base}>{textOf(data[0], x)}</text>
          <text className={r.label.base}>{textOf(data[data.length - 1], x)}</text>
        </view>
      )}
    >
      {scale === 'ends' && <text aria-hidden className={r.scale.base}>{formatValue(high)}</text>}
      <view role={isPressable ? undefined : 'list'} aria-label={isPressable ? undefined : label} className={cx(r.plot.base, scale === 'ends' && r.plot.ruled, scale === 'ends' && isSigned && r.plot.floored)}>
        {/* The zero line is laid out as a column is, so it sits where the bars meet. */}
        <view aria-hidden className={r.baseline.base}>
          {scale === 'each' && <view className={r.valueSlot.base} />}
          <view className={r.baseline.above} style={{ flexGrow: high + depth > 0 ? high : 1 }} />
          <view className={r.space.base} style={{ flexGrow: depth }} />
          {scale === 'each' && isSigned && <view className={r.valueSlot.base} />}
        </view>
        {slots}
      </view>
      {scale === 'ends' && isSigned && <text aria-hidden className={r.scale.base}>{formatValue(-depth)}</text>}
    </Chart>
  );
}
