import { cx, type VectorPath } from '@lingxia/view';
import { Chart, ChartLegend, ChartReadout, type ChartLegendPosition, type ChartRow, type ChartSeries, type ChartSize, type ChartSummarize } from '../Chart/Chart';
import { colorOf, keyOf, numberOf, plain, roleOf, seriesFacts, seriesOf, textOf } from '../internal/data';
import { box, closed, domain, dot, monotone, polyline, roundRange, runs, scaleY, thin, type Point } from '../internal/geometry';
import { lineChartRecipe as r } from './LineChart.recipe';

export interface LineChartProps {
  /** The rows, one per point in time, oldest first. */
  data: readonly ChartRow[];
  /** The field that names each point: "month". */
  x: string;
  /** The field that holds the number, or up to five series drawn as separate lines. A row without a number is a gap. */
  y: string | readonly ChartSeries[];
  /** The field that identifies a row across updates and in `selectedKey`. By default `x`. */
  keyField?: string;
  /** The chart's accessible name: "Weight by week". */
  label: string;
  title?: string;
  description?: string;
  /** Turns a number into the text shown and read. By default the bare number. */
  formatValue?: (value: number) => string;
  /** Writes the summary a screen reader hears from the chart's facts; see `@liulidesign/charts/summary/en`. */
  summarize?: ChartSummarize;
  /** Rounds the line between points without passing above or below them. @default false */
  isSmooth?: boolean;
  /** Fills under the line and measures it from zero. One series only. @default false */
  hasArea?: boolean;
  /** Marks each line's latest value with a dot. @default true */
  hasEndDot?: boolean;
  /**
   * Shows the scale: the top and bottom of the value range, as round numbers, at the start edge
   * with a hairline at the top. By default on, except at the `xs` size.
   */
  hasScale?: boolean;
  /** Writes the latest value at the end of the line. One series only. @default false */
  hasEndLabel?: boolean;
  /** The first and last category under the plot, or none. By default `ends`, except at the `xs` size. */
  xLabels?: 'ends' | 'none';
  /** Shown when there is more than one series. @default 'bottom' */
  legend?: ChartLegendPosition;
  /** The legend's accessible name; by default the chart's `label`. */
  legendLabel?: string;
  /** The row to mark with a rule and a dot on each line, and to read out above the plot. */
  selectedKey?: string | null;
  /** Series fields not drawn at present. @default [] */
  hiddenSeries?: readonly string[];
  /** Makes each legend entry a button that shows or hides its series. */
  onHiddenSeriesChange?: (fields: readonly string[]) => void;
  /** Widens the value range downward. A line fits its data; an area starts from zero. */
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
  /** On the chart. */
  'data-testid'?: string;
}

// The second and later lines differ by dash as well as colour, so they survive without colour.
const dashes: readonly (readonly number[])[] = [[], [6, 4], [2, 3], [9, 3, 2, 3], [4, 2]];
const strokeWidth = 2;
const dotWidth = 6;
const selectedWidth = 8;
// Past this many rows a screen reader gets the summary alone; a longer list is not a reading.
const maxRead = 60;

/** A number over time, as a line that fits its data, or as an area measured from zero. */
export function LineChart({
  data, x, y, keyField, label, title, description, formatValue: format, summarize, isSmooth = false, hasArea = false, hasEndDot = true,
  hasScale, hasEndLabel = false, xLabels, legend = 'bottom', legendLabel, selectedKey, hiddenSeries = [], onHiddenSeriesChange,
  domainMin, domainMax, size = 'md', isLoading = false, error, emptyLabel, className, 'data-testid': testId,
}: LineChartProps) {
  const formatValue = format ?? plain;
  const all = seriesOf(y).map((series, index) => ({ ...series, color: colorOf(index, series.color), dash: dashes[index % dashes.length] }));
  const shown = all.filter(series => !hiddenSeries.includes(series.field));
  const facts = shown.flatMap(series => seriesFacts(data, x, series, formatValue) ?? []);
  const columns = shown.map(series => data.map(row => numberOf(row, series.field)));
  const isArea = hasArea && shown.length === 1;
  const isScaled = hasScale ?? size !== 'xs';
  const fitted = domain(columns.flat(), isArea, domainMin, domainMax);
  // With the scale written out, the plot's top and bottom are round numbers.
  const range = isScaled && facts.length ? roundRange(fitted) : fitted;
  const latest = shown.length === 1 && hasEndLabel ? facts[0]?.last : undefined;
  const ends = (xLabels ?? (size === 'xs' ? 'none' : 'ends')) === 'ends' && data.length > 1;
  // A scale label that says what the end figure says, on the same line, is left to the end figure.
  const scale = isScaled && facts.length > 0 && [formatValue(range[1]), formatValue(range[0])].map(text => (text === latest?.text ? '' : text));
  const chosen = data.findIndex((row, index) => keyOf(row, keyField ?? x, index) === selectedKey);
  const picked = chosen >= 0 ? data[chosen] : undefined;

  const paths: VectorPath[] = [];
  const trace = (points: readonly Point[]) => (isSmooth ? monotone(points) : polyline(points));
  const round = { fill: 'none', strokeLinecap: 'round', strokeLinejoin: 'round', vectorEffect: 'non-scaling-stroke' } as const;
  if (picked) paths.push({ d: `M${(chosen / Math.max(1, data.length - 1)) * box} 0V${box}`, stroke: 'text/20', strokeWidth: 1, fill: 'none', vectorEffect: 'non-scaling-stroke' });
  shown.forEach((series, index) => {
    const stroke = roleOf(series.color);
    const lines = runs(thin(columns[index]), range);
    const joined = lines.filter(points => points.length > 1);
    if (isArea && joined.length) paths.push({ d: joined.map(points => closed(trace(points), points, box)).join(''), fill: `${stroke}/16` });
    if (joined.length) paths.push({
      d: joined.map(trace).join(''), stroke, strokeWidth, ...round,
      ...(series.dash.length ? { strokeDasharray: series.dash, strokeLinecap: 'butt' } : {}),
    });
    const alone = lines.filter(points => points.length === 1);
    if (alone.length) paths.push({ d: alone.map(points => dot(points[0])).join(''), stroke, strokeWidth: strokeWidth * 2, ...round });
    const latest = lines[lines.length - 1];
    if (hasEndDot && latest) paths.push({ d: dot(latest[latest.length - 1]), stroke, strokeWidth: dotWidth, ...round });
    const value = picked ? numberOf(picked, series.field) : NaN;
    if (Number.isFinite(value))
      paths.push({ d: dot([(chosen / Math.max(1, data.length - 1)) * box, scaleY(value, range)]), stroke, strokeWidth: selectedWidth, ...round });
  });

  const reading = (row: ChartRow) => shown
    .filter(series => Number.isFinite(numberOf(row, series.field)))
    .map(series => (series.label ? `${series.label} ${formatValue(numberOf(row, series.field))}` : formatValue(numberOf(row, series.field))))
    .join(', ');

  return (
    <Chart
      label={label}
      title={title}
      description={description}
      summary={summarize?.({ kind: 'line', count: data.length, series: facts })}
      size={size}
      isLoading={isLoading}
      error={error}
      isEmpty={!facts.length}
      emptyLabel={emptyLabel}
      className={className}
      data-testid={testId}
      readout={picked && (
        <ChartReadout
          label={textOf(picked, x)}
          values={shown.filter(series => Number.isFinite(numberOf(picked, series.field))).map(series => ({
            text: formatValue(numberOf(picked, series.field)), ...(shown.length > 1 ? { label: series.label, color: series.color } : {}),
          }))}
        />
      )}
      legendPosition={legend}
      legend={all.length > 1 && (
        <ChartLegend
          label={legendLabel ?? label}
          mark="line"
          items={all.map(series => ({ key: series.field, label: series.label, color: series.color, dash: series.dash, isOff: hiddenSeries.includes(series.field) }))}
          pressedState="shown"
          onItemPress={onHiddenSeriesChange && (field => onHiddenSeriesChange(hiddenSeries.includes(field) ? hiddenSeries.filter(each => each !== field) : [...hiddenSeries, field]))}
          data-testid={testId === undefined ? undefined : `${testId}-legend`}
        />
      )}
      axis={ends ? (
        <view className={r.labels.base}>
          {/* The plot is narrower than the chart by the scale and the end figure; the same text, unseen, insets these labels as much. */}
          {scale && <view className={cx(r.scale.base, r.ghost.base)}>{scale.map((text, index) => <text key={String(index)} className={r.scaleLabel.base}>{text}</text>)}</view>}
          <view className={r.labels.ends}>
            <text className={r.label.base}>{textOf(data[0], x)}</text>
            <text className={r.label.base}>{textOf(data[data.length - 1], x)}</text>
          </view>
          {latest && <view className={cx(r.end.base, r.ghost.base)}><text className={r.endLabel.base}>{latest.text}</text></view>}
        </view>
      ) : undefined}
    >
      <view className={r.frame.base}>
        {scale && (
          <view aria-hidden className={cx(r.scale.base, r.inset.base, !isArea && r.inset.open)}>
            <text className={cx(r.scaleLabel.base, r.scaleLabel.top)}>{scale[0]}</text>
            <text className={cx(r.scaleLabel.base, r.scaleLabel.bottom)}>{scale[1]}</text>
          </view>
        )}
        <view className={r.plot.base}>
          {scale && <view aria-hidden className={r.rule.top} />}
          <view aria-hidden className={cx(r.rule.bottom, !isArea && r.rule.open)} />
          {paths.length > 0 && <vector aria-hidden paths={paths} viewBox={[0, 0, box, box]} className={cx(r.drawing.base, r.inset.base, !isArea && r.inset.open)} />}
          {data.length <= maxRead && (
            <view role="list" aria-label={label} className={r.reading.base}>
              {data.map((row, index) => reading(row) !== '' && <text key={keyOf(row, keyField ?? x, index)} role="listitem">{`${textOf(row, x)}: ${reading(row)}`}</text>)}
            </view>
          )}
        </view>
        {latest && (
          <view aria-hidden className={cx(r.end.base, r.inset.base, !isArea && r.inset.open)}>
            <view className={r.space.base} style={{ flexGrow: range[1] - latest.value }} />
            <view className={r.endAnchor.base}><text className={cx(r.endLabel.base, r.endLabel.level)}>{latest.text}</text></view>
            <view className={r.space.base} style={{ flexGrow: latest.value - range[0] }} />
          </view>
        )}
      </view>
    </Chart>
  );
}
