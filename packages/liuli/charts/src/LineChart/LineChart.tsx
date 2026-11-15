import { cx, type PointerEvent, type VectorPath } from '@lingxia/view';
import { Chart, ChartLegend, ChartReadout, type ChartLegendPosition, type ChartRow, type ChartSeries, type ChartSize, type ChartSummarize } from '../Chart/Chart';
import { colorOf, keyOf, numberOf, plain, roleOf, seriesFacts, seriesOf, textOf } from '../internal/data';
import { pressed, pressedKey, scrubbing } from '../internal/gesture';
import { box, closed, domain, dot, indexAt, monotone, polyline, roundRange, runs, scaleY, thin, type Point } from '../internal/geometry';
import { Ticks } from '../internal/Ticks';
import { remember } from '../internal/memo';
import { lineChartRecipe as r } from './LineChart.recipe';

export type LineChartLabels = 'fit' | 'ends' | 'none';

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
  /**
   * Category labels under the plot: `fit` writes as many as fit side by side, each under its
   * point; `ends` the first and last. By default `fit`, except at the `xs` size.
   */
  xLabels?: LineChartLabels;
  /** Shown when there is more than one series. @default 'bottom' */
  legend?: ChartLegendPosition;
  /** The legend's accessible name; by default the chart's `label`. */
  legendLabel?: string;
  /** The row to mark with a rule and a dot on each line, and to read out above the plot. */
  selectedKey?: string | null;
  /**
   * Makes the plot a scrubber: a finger dragged across it, a tap, a mouse moving over it and the
   * arrow keys choose the nearest row and run this with its key; a mouse leaving runs it with `null`.
   */
  onSelect?: (key: string | null) => void;
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
// Half a marker: the drawing's first and last points sit this far inside the plot.
const edge = 4;
// Past this many rows a screen reader gets the summary alone; a longer list is not a reading.
const maxRead = 60;

/** A number over time, as a line that fits its data, or as an area measured from zero. */
export function LineChart({
  data, x, y, keyField, label, title, description, formatValue: format, summarize, isSmooth = false, hasArea = false, hasEndDot = true,
  hasScale, hasEndLabel = false, xLabels, legend = 'bottom', legendLabel, selectedKey, onSelect, hiddenSeries = [], onHiddenSeriesChange,
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
  const labels = data.length > 1 ? xLabels ?? (size === 'xs' ? 'none' : 'fit') : 'none';
  // A scale label that says what the end figure says, on the same line, is left to the end figure.
  const scale = isScaled && facts.length > 0 && [formatValue(range[1]), formatValue(range[0])].map(text => (text === latest?.text ? '' : text));
  const chosen = data.findIndex((row, index) => keyOf(row, keyField ?? x, index) === selectedKey);
  const picked = chosen >= 0 ? data[chosen] : undefined;

  // The lines depend on the data and the encoding, never on the theme or the chosen row, so a scrub redraws only the marker.
  const paths = remember(data, 'lines', JSON.stringify([shown.map(series => [series.field, series.color, series.dash]), range, isSmooth, isArea, hasEndDot]), () => {
    const paths: VectorPath[] = [];
    const trace = (points: readonly Point[]) => (isSmooth ? monotone(points) : polyline(points));
    const round = { fill: 'none', strokeLinecap: 'round', strokeLinejoin: 'round', vectorEffect: 'non-scaling-stroke' } as const;
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
    });
    return paths;
  });

  // The chosen row: a rule down the plot and a dot on each line.
  const marker: VectorPath[] = [];
  if (picked) {
    const at = (chosen / Math.max(1, data.length - 1)) * box;
    const round = { fill: 'none', strokeLinecap: 'round', vectorEffect: 'non-scaling-stroke' } as const;
    marker.push({ d: `M${at} 0V${box}`, stroke: 'text/20', strokeWidth: 1, fill: 'none', vectorEffect: 'non-scaling-stroke' });
    for (const series of shown) {
      const value = numberOf(picked, series.field);
      if (Number.isFinite(value)) marker.push({ d: dot([at, scaleY(value, range)]), stroke: roleOf(series.color), strokeWidth: selectedWidth, ...round });
    }
  }

  const reading = (row: ChartRow) => shown
    .filter(series => Number.isFinite(numberOf(row, series.field)))
    .map(series => (series.label ? `${series.label} ${formatValue(numberOf(row, series.field))}` : formatValue(numberOf(row, series.field))))
    .join(', ');

  const named = (row: ChartRow) => (reading(row) === '' ? textOf(row, x) : `${textOf(row, x)}: ${reading(row)}`);
  // With nothing chosen the scrubber rests on the latest row, which is what the line's end shows.
  const rest = chosen >= 0 ? chosen : data.length - 1;
  const scrub = onSelect && facts.length > 0 ? (event: PointerEvent) => {
    const { phase } = event;
    if (phase === 'leave') return chosen >= 0 ? onSelect(null) : undefined;
    const current = chosen >= 0 ? keyOf(data[chosen], keyField ?? x, chosen) : null;
    if (phase === 'down') pressed(event, -1, current);
    // The page took the press for a scroll: what it chose on the way is put back.
    if (phase === 'cancel') return pressedKey() === current ? undefined : onSelect(pressedKey());
    // A finger chooses once it moves sideways or lifts, never as it lands.
    if ((phase === 'down' && event.pointer === 'touch') || (phase === 'move' && !scrubbing(event))) return undefined;
    const index = phase === 'increment' ? Math.min(data.length - 1, rest + 1)
      : phase === 'decrement' ? Math.max(0, rest - 1)
        : indexAt(event.x, event.width, data.length, 'points', edge);
    return index === chosen ? undefined : onSelect(keyOf(data[index], keyField ?? x, index));
  } : undefined;

  const ends = (className: string) => (
    <view className={className}>
      <text className={r.label.base}>{textOf(data[0], x)}</text>
      <text className={r.label.base}>{textOf(data[data.length - 1], x)}</text>
    </view>
  );

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
      // A chart that can be scrubbed keeps the readout's line, so choosing a row moves nothing.
      readout={(picked || scrub) && (
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
          mark="line"
          items={all.map(series => ({ key: series.field, label: series.label, color: series.color, dash: series.dash, isOff: hiddenSeries.includes(series.field) }))}
          pressedState="shown"
          onItemPress={onHiddenSeriesChange && (field => onHiddenSeriesChange(hiddenSeries.includes(field) ? hiddenSeries.filter(each => each !== field) : [...hiddenSeries, field]))}
          data-testid={testId === undefined ? undefined : `${testId}-legend`}
        />
      )}
      axis={labels !== 'none' ? (
        <view className={r.labels.base}>
          {/* The plot is narrower than the chart by the scale and the end figure; the same text, unseen, insets these labels as much. */}
          {scale && <view className={cx(r.scale.base, r.ghost.base)}>{scale.map((text, index) => <text key={String(index)} className={r.scaleLabel.base}>{text}</text>)}</view>}
          {labels === 'fit' ? <Ticks texts={data.map(row => textOf(row, x))} layout="points" inset={edge}>{ends(r.labels.cover)}</Ticks> : ends(r.labels.ends)}
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
          {marker.length > 0 && <vector aria-hidden paths={marker} viewBox={[0, 0, box, box]} className={cx(r.drawing.base, r.drawing.over, r.inset.base, !isArea && r.inset.open)} />}
          {/* Over the marks: it hears the pointer, and is the stop the arrow keys and a screen reader step. */}
          {scrub && (
            <view
              onPointer={scrub}
              pointerAxis="x"
              aria-label={label}
              aria-valuenow={rest}
              aria-valuemin={0}
              aria-valuemax={data.length - 1}
              aria-valuetext={named(data[rest])}
              data-testid={testId === undefined ? undefined : `${testId}-plot`}
              className={r.scrubber.base}
            />
          )}
          {data.length <= maxRead && (
            <view role="list" aria-label={label} className={r.reading.base}>
              {data.map((row, index) => reading(row) !== '' && <text key={keyOf(row, keyField ?? x, index)} role="listitem">{named(row)}</text>)}
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
