import { cx, useBox, type Child, type PointerEvent, type VectorPath } from '@lingxia/view';
import { Chart, ChartLegend, ChartReadout, type ChartLegendPosition, type ChartRow, type ChartSeries, type ChartSize, type ChartSummarize } from '../Chart/Chart';
import { colorOf, keyOf, numberOf, plain, roleOf, seriesFacts, seriesOf, textOf } from '../internal/data';
import { pressed, pressedKey, scrubbing } from '../internal/gesture';
import { box, closed, domain, dot, indexAt, monotone, niceTicks, polyline, reach, runs, scaleY, shares, thin, type Point } from '../internal/geometry';
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
   * Shows the scale: round numbers at the start edge, each with a hairline across the plot. The
   * range reaches just past the data, so the plot is filled. By default on, except at the `xs` size.
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
  /** A second, quieter line under `emptyLabel`: what to do next. */
  emptyHint?: string;
  /** With `error`: tries again. Pass `retryLabel` with it. */
  onRetry?: () => void;
  /** The words on the retry button: "Try again". */
  retryLabel?: string;
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
// The plot's height at each size, until it has been measured.
const heights = { xs: 64, sm: 112, md: 176, lg: 240 } as const;
// Past this many rows a screen reader gets the summary alone; a longer list is not a reading.
const maxRead = 60;

/** A number over time, as a line that fits its data, or as an area measured from zero. */
export function LineChart({
  data, x, y, keyField, label, title, description, formatValue: format, summarize, isSmooth = false, hasArea = false, hasEndDot = true,
  hasScale, hasEndLabel = false, xLabels, legend = 'bottom', legendLabel, selectedKey, onSelect, hiddenSeries = [], onHiddenSeriesChange,
  domainMin, domainMax, size = 'md', isLoading = false, error, emptyLabel, emptyHint, onRetry, retryLabel, className, 'data-testid': testId,
}: LineChartProps) {
  const formatValue = format ?? plain;
  const all = seriesOf(y).map((series, index) => ({ ...series, color: colorOf(index, series.color), dash: dashes[index % dashes.length] }));
  const shown = all.filter(series => !hiddenSeries.includes(series.field));
  const facts = shown.flatMap(series => seriesFacts(data, x, series, formatValue) ?? []);
  const columns = shown.map(series => data.map(row => numberOf(row, series.field)));
  const isArea = hasArea && shown.length === 1;
  const isScaled = hasScale ?? size !== 'xs';
  const fitted = domain(columns.flat(), isArea, domainMin, domainMax);
  // With the scale written out, the plot's lines sit on round numbers: fewer of them in a short plot.
  const scaled = isScaled && facts.length ? niceTicks(fitted, size === 'sm' ? 2 : 4) : undefined;
  const range = scaled?.range ?? fitted;
  const latest = shown.length === 1 && hasEndLabel ? facts[0]?.last : undefined;
  const labels = data.length > 1 ? xLabels ?? (size === 'xs' ? 'none' : 'fit') : 'none';
  // The scale from the top down. A label that says what the end figure says, on the same line, is left to the end figure.
  const scale = scaled && [...scaled.ticks].reverse().map(value => ({ value, text: formatValue(value) === latest?.text ? '' : formatValue(value) }));
  // Each tick's place: the plot above the first, between two, and under the last, as the scale counted them.
  const gaps = scaled?.gaps ?? [];
  // The end figure's place: the plot above its value and under it.
  const [aboveLatest, underLatest] = latest ? shares(range[1] - latest.value, latest.value - range[0]) : [1, 1];
  const rungs = (mark: (tick: { value: number; text: string }, index: number) => Child) => scale && scale.flatMap((tick, index) => [
    <view key={`gap-${index}`} className={r.space.base} style={{ flexGrow: gaps[index] }} />,
    <view key={`tick-${index}`} className={r.tick.base}>{mark(tick, index)}</view>,
  ]).concat(<view key="foot" className={r.space.base} style={{ flexGrow: gaps[gaps.length - 1] }} />);
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

  // A drawing is cut at its viewBox, so the viewBox reaches half a marker past the data on each
  // side that has one: a dot on the first, last, highest or lowest value is whole.
  const plot = useBox('plot');
  const frame = reach(plot.value?.width ?? 300, plot.value?.height ?? heights[size], edge, isArea ? 0 : edge);

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
      plotShape="line"
      error={error}
      onRetry={onRetry}
      retryLabel={retryLabel}
      emptyHint={emptyHint}
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
          {scale && <view className={cx(r.scale.base, r.ghost.base)}>{scale.map((tick, index) => <text key={String(index)} className={r.scaleLabel.base}>{tick.text}</text>)}</view>}
          {labels === 'fit' ? <Ticks texts={data.map(row => textOf(row, x))} layout="points" inset={edge}>{ends(r.labels.cover)}</Ticks> : ends(r.labels.ends)}
          {latest && <view className={cx(r.end.base, r.ghost.base)}><text className={r.endLabel.base}>{latest.text}</text></view>}
        </view>
      ) : undefined}
    >
      <view className={r.frame.base}>
        {scale && (
          <view aria-hidden className={cx(r.scale.base, r.inset.base, !isArea && r.inset.open)}>
            {rungs(tick => <text className={cx(r.scaleLabel.base, r.scaleLabel.level)}>{tick.text}</text>)}
          </view>
        )}
        <view box={plot} className={r.plot.base}>
          {scale
            // A hairline on each tick; the one the values stand on is firmer.
            ? <view aria-hidden className={cx(r.grid.base, r.inset.base, !isArea && r.inset.open)}>{rungs(tick => <view className={tick.value === range[0] ? r.rule.foot : r.rule.line} />)}</view>
            : <view aria-hidden className={cx(r.rule.bottom, !isArea && r.rule.open)} />}
          {paths.length > 0 && <vector aria-hidden paths={paths} viewBox={frame} className={r.drawing.base} />}
          {marker.length > 0 && <vector aria-hidden paths={marker} viewBox={frame} className={cx(r.drawing.base, r.drawing.over)} />}
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
            <view className={r.space.base} style={{ flexGrow: aboveLatest }} />
            <view className={r.endAnchor.base}><text className={cx(r.endLabel.base, r.endLabel.level)}>{latest.text}</text></view>
            <view className={r.space.base} style={{ flexGrow: underLatest }} />
          </view>
        )}
      </view>
    </Chart>
  );
}
