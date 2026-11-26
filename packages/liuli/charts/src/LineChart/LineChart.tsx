import { cx, useBox, useLocal, useView, type Child, type PointerEvent, type VectorPath, type Local } from '@lingxia/view';
import { Chart, ChartLegend, ChartReadout, type ChartLegendPosition, type ChartField, type ChartRecord, type ChartReference, type ChartRow, type ChartSeries, type ChartSize, type ChartSummarize } from '../Chart/Chart';
import { colorOf, keysOf, numberOf, numberIn, referenceEnds, referenceTop, roleOf, seriesFacts, seriesOf, textOf, type ChartColor } from '../internal/data';
import { pressed, pressedKey, scrubbing } from '../internal/gesture';
import { between, box, closed, domain, dot, indexAt, niceTicks, reach, runs, scaleY, thin, traced, type ChartCurve, type Point } from '../internal/geometry';
import { ReferenceLabels, ReferenceReading, referencePath, useReferences, zonePath } from '../internal/References';
import { Ticks } from '../internal/Ticks';
import { laidOutOnce, remember } from '../internal/memo';
import { lineChartRecipe as r } from './LineChart.recipe';

export type { ChartCurve } from '../internal/geometry';
export type LineChartLabels = 'fit' | 'ends' | 'none';

/** `Row` is the app's own record type, so `x`, `y` and `keyField` name its fields and a typo fails to compile. */
export interface LineChartProps<Row extends ChartRecord<Row> = ChartRow> {
  /** The rows, one per point in time, oldest first. */
  rows: readonly Row[];
  /** The field that names each point: "month". */
  x: ChartField<Row>;
  /** The field that holds the number, or up to five series drawn as separate lines. A row without a number is a gap. */
  y: ChartField<Row> | readonly ChartSeries<Row>[];
  /** The field that identifies a row across updates and in `value`. By default `x`. */
  keyField?: ChartField<Row>;
  /** The chart's accessible name: "Weight by week". */
  label: string;
  title?: string;
  description?: string;
  /** Turns a number into the text shown and read. By default the number as the page's language writes it, grouped (`Intl.NumberFormat`). */
  formatValue?: (value: number) => string;
  /** Writes the summary a screen reader hears from the chart's facts; see `@liulidesign/charts/summary/en`. */
  summarize?: ChartSummarize;
  /** How the line goes between points: `smooth` rounds it without passing above or below them; `step` holds each value level across its point and changes halfway to the next. @default 'straight' */
  curve?: ChartCurve;
  /** Fills under each line and measures it from zero. @default false */
  hasArea?: boolean;
  /** Shades the range between two fields behind the lines: a day's low and high, a forecast's margin. Each row reads it as `label low–high`. */
  band?: LineChartBand<Row>;
  /** `stacked` lays each series on the ones before it, as filled bands whose top line is the running total; the values read stay each series' own. @default 'overlaid' */
  layout?: LineChartLayout;
  /**
   * Marks each line's latest value with a dot. By default only with the end label, which the dot
   * ties to its line; a lone dot at rest reads as a chosen point. @default hasEndLabel
   */
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
  legendPosition?: ChartLegendPosition;
  /** The legend's accessible name; by default the chart's `label`. */
  legendLabel?: string;
  /** The row to mark with a rule and a dot on each line, and to read out above the plot. */
  value?: string | null;
  /**
   * Makes the plot a scrubber: a finger dragged across it, a tap, a mouse moving over it and the
   * arrow keys choose the nearest row and run this with its key; a mouse leaving runs it with `null`.
   */
  onChange?: (key: string | null) => void;
  /** Where the choice starts when the chart keeps it itself: give this instead of `value`; `onChange` still hears each choice. */
  defaultValue?: string | null;
  /** A View-local cell holding the chosen key, read and written instead of `value` with `onChange`; `onChange` still hears each choice. */
  bind?: Local<string | null>;
  /** Series fields not drawn at present. @default [] */
  hiddenSeries?: readonly string[];
  /** Makes each legend entry a button that shows or hides its series. */
  onHiddenSeriesChange?: (fields: readonly string[]) => void;
  /** The series hidden at first when the chart keeps them itself: give this instead of `hiddenSeries`; the legend entries then toggle them, and `onHiddenSeriesChange` still hears each change. */
  defaultHiddenSeries?: readonly string[];
  /**
   * Levels drawn across the plot as dashed lines, each named at the end edge with its value: a target,
   * an average. The range widens to hold them. @default []
   */
  references?: readonly ChartReference[];
  /** Widens the value range downward. A line fits its data; an area starts from zero. */
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
// Kept clear around a line's last point, past its dot: the latest value is what people look at first.
const endClear = dotWidth / 2 + 3;
// The plot's height at each size, until it has been measured.
const heights = { xs: 64, sm: 112, md: 176, lg: 240 } as const;
// Past this many rows a screen reader gets the summary alone; a longer list is not a reading.
const maxRead = 60;

export type LineChartLayout = 'overlaid' | 'stacked';

export interface LineChartBand<Row = ChartRow> {
  low: ChartField<Row>;
  high: ChartField<Row>;
  /** What the range is: "Range", "Forecast". */
  label: string;
  /** Its colour; by default the first series'. */
  color?: ChartColor;
}

/** A number over time, as a line that fits its data, or as an area measured from zero. */
export function LineChart<Row extends ChartRecord<Row>>({
  rows, x, y, keyField, label, title, description, formatValue: format, summarize, curve = 'straight', hasArea = false, band, layout = 'overlaid',
  hasScale, hasEndLabel = false, hasEndDot = hasEndLabel, xLabels, legendPosition = 'bottom', legendLabel, value, onChange, defaultValue, bind, hiddenSeries: givenHidden, onHiddenSeriesChange: onHiddenChange, defaultHiddenSeries,
  references: given = [], domainMin, domainMax, size = 'md', isLoading = false, error, emptyLabel, emptyHint, onRetry, retryLabel, className, 'data-testid': testId,
}: LineChartProps<Row>) {
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
  const formatValue = format ?? numberIn(useView().env.language);
  const references = given.filter(each => Number.isFinite(each.value));
  const all = seriesOf(y).map((series, index) => ({ ...series, color: colorOf(index, series.color), dash: dashes[index % dashes.length] }));
  const shown = all.filter(series => !hiddenSeries.includes(series.field));
  const facts = shown.flatMap(series => seriesFacts(data, x, series, formatValue) ?? []);
  const isStacked = layout === 'stacked' && shown.length > 1;
  // Stacked, each series is drawn at the running total, a gap counting as nothing.
  const columns = shown.map(series => data.map(row => numberOf(row, series.field)));
  const drawnColumns = isStacked
    ? columns.map((_, index) => data.map((__, row) => columns.slice(0, index + 1).reduce((sum, column) => sum + (Number.isFinite(column[row]) ? column[row] : 0), 0)))
    : columns;
  const isArea = hasArea || isStacked;
  // A band is drawn only where both its ends are known.
  const bandColumns = band ? (() => {
    const [low, high] = [band.low, band.high].map(field => data.map(row => numberOf(row, field)));
    const known = (index: number) => Number.isFinite(low[index]) && Number.isFinite(high[index]);
    return [low, high].map(column => column.map((value, index) => (known(index) ? value : NaN)));
  })() : [];
  const isScaled = hasScale ?? size !== 'xs';
  const fitted = domain([...drawnColumns.flat(), ...bandColumns.flat(), ...references.flatMap(referenceEnds)], isArea, domainMin, domainMax);
  // With the scale written out, the plot's lines sit on round numbers: fewer of them in a short plot.
  const scaled = isScaled && facts.length ? niceTicks(fitted, size === 'sm' ? 2 : 4) : undefined;
  const range = scaled?.range ?? fitted;
  const latest = shown.length === 1 && hasEndLabel ? facts[0]?.last : undefined;
  const labels = data.length > 1 ? xLabels ?? (size === 'xs' ? 'none' : 'fit') : 'none';
  // The scale from the top down. A label that says what the end figure says, on the same line, is left to the end figure.
  const scale = scaled && [...scaled.ticks].reverse().map(value => ({ value, text: formatValue(value) === latest?.text ? '' : formatValue(value) }));
  // A label's place: the value's height on the drawing's own scale, as a share of a box as tall as
  // the plot. The box moves on the lines' timing, so a label stays level with what it names.
  const level = (value: number): { translate: readonly [number, `${number}%`] } => ({ translate: [0, `${(scaleY(value, range) / box) * 100}%`] });
  const rungs = (mark: (tick: { value: number; text: string }) => Child) => scale && (
    <view className={r.track.base}>
      {/* Keyed by value: a tick that stays when the range changes moves to its new place. */}
      {scale.map(tick => <view key={`tick-${tick.value}`} className={cx(r.mover.base, r.mover.tick)} style={level(tick.value)}><view className={r.tick.base}>{mark(tick)}</view></view>)}
    </view>
  );
  const keys = keysOf(data, keyField ?? x);
  const chosen = selectedKey == null ? -1 : keys.indexOf(selectedKey);
  const picked = chosen >= 0 ? data[chosen] : undefined;

  // The lines depend on the data and the encoding, never on the theme or the chosen row, so a scrub redraws only the marker.
  const paths = remember(data, 'lines', JSON.stringify([shown.map(series => [series.field, series.color, series.dash]), range, curve, isArea, isStacked, hasEndDot, band && [band.low, band.high, band.color ?? all[0]?.color]]), () => {
    const paths: VectorPath[] = [];
    const trace = (points: readonly Point[]) => traced(points, curve);
    const round = { fill: 'none', strokeLinecap: 'round', strokeLinejoin: 'round', vectorEffect: 'non-scaling-stroke' } as const;
    if (band) {
      const [lows, highs] = bandColumns.map(column => runs(thin(column), range));
      const d = highs.map((top, index) => (top.length > 1 ? between(top, lows[index], curve) : '')).join('');
      if (d) paths.push({ key: 'band-area', d, fill: `${roleOf(band.color ?? all[0]?.color ?? 1)}/16` });
    }
    shown.forEach((series, index) => {
      const stroke = roleOf(series.color);
      const lines = runs(thin(drawnColumns[index]), range);
      const joined = lines.filter(points => points.length > 1);
      // A stacked band lies between this total and the one under it, a shade firmer so neighbours part.
      const floor = index > 0 && isStacked ? runs(thin(drawnColumns[index - 1]), range)[0] : undefined;
      if (isArea && joined.length) paths.push({
        key: `${series.field}-area`,
        d: floor ? joined.map(points => between(points, floor, curve)).join('') : joined.map(points => closed(trace(points), points, scaleY(0, range))).join(''),
        fill: `${stroke}/${isStacked ? 24 : 16}`,
      });
      if (joined.length) paths.push({
        key: `${series.field}-line`, d: joined.map(trace).join(''), stroke, strokeWidth, ...round,
        ...(series.dash.length ? { strokeDasharray: series.dash, strokeLinecap: 'butt' } : {}),
      });
      const alone = lines.filter(points => points.length === 1);
      if (alone.length) paths.push({ key: `${series.field}-alone`, d: alone.map(points => dot(points[0])).join(''), stroke, strokeWidth: strokeWidth * 2, ...round });
      const latest = lines[lines.length - 1];
      if (hasEndDot && latest) paths.push({ key: `${series.field}-end`, d: dot(latest[latest.length - 1]), stroke, strokeWidth: dotWidth, ...round });
    });
    return paths;
  });

  // The chosen row: a rule down the plot and a dot on each line.
  const marker: VectorPath[] = [];
  if (picked) {
    // One row sits in the middle, where its point is drawn.
    const at = data.length > 1 ? (chosen / (data.length - 1)) * box : box / 2;
    const round = { fill: 'none', strokeLinecap: 'round', vectorEffect: 'non-scaling-stroke' } as const;
    // Keyed by the row: another row is a new marker, drawn at once; new data under the same row moves it with the lines.
    marker.push({ key: `rule-${chosen}`, d: `M${at} 0V${box}`, stroke: 'text/20', strokeWidth: 1, fill: 'none', vectorEffect: 'non-scaling-stroke' });
    shown.forEach((series, index) => {
      const value = numberOf(picked, series.field);
      if (Number.isFinite(value)) marker.push({ key: `${series.field}-${chosen}`, d: dot([at, scaleY(drawnColumns[index][chosen], range)]), stroke: roleOf(series.color), strokeWidth: selectedWidth, ...round });
    });
  }

  // A drawing is cut at its viewBox, so the viewBox reaches half a marker past the data on each
  // side that has one: a dot on the first, last, highest or lowest value is whole.
  const plot = useBox('plot');
  const frame = reach(plot.value?.width ?? 300, plot.value?.height ?? heights[size], edge, isArea ? 0 : edge);
  const foot = isArea ? 0 : edge;
  const height = (value: number) => edge + (scaleY(value, range) / box) * ((plot.value?.height ?? 0) - edge - foot);
  // Each line's last point, as a box a reference label keeps clear of.
  const laid = plot.value;
  const lastPoints = laid ? drawnColumns.flatMap(column => {
    const last = column.reduce((found, value, index) => (Number.isFinite(value) ? index : found), -1);
    if (last < 0) return [];
    const across = data.length > 1 ? edge + (last / (data.length - 1)) * (laid.width - edge * 2) : laid.width / 2;
    const y = height(column[last]);
    return [{ start: across - endClear, end: across + endClear, top: y - endClear, bottom: y + endClear }];
  }) : [];
  const refs = useReferences(references, formatValue, height, laid, lastPoints);
  const referenceLines = references.flatMap((each, index) => referenceEnds(each).map((end, at) => referencePath(at ? index + references.length : index, scaleY(end, range), frame[0], frame[0] + frame[2])));
  // A zone's shade lies under the lines; its edges, like a level, over them.
  const zones = references.flatMap((each, index) => (referenceEnds(each).length > 1 ? [zonePath(index, scaleY(each.value, range), scaleY(each.to!, range), frame[0], frame[0] + frame[2])] : []));

  // A line draws in along its length; a fill cannot follow it, so fills, and the dots that end the
  // lines, wait in their own layers and fade in as the line arrives.
  const drawn = laidOutOnce(data, `line:${label}`, plot.value !== null);
  const under = paths.filter(path => path.key?.endsWith('-area'));
  const lines = paths.filter(path => path.key?.endsWith('-line'));
  const over = paths.filter(path => path.key?.endsWith('-alone') || path.key?.endsWith('-end'));
  const layer = (list: VectorPath[], key: string) => list.length > 0 && (
    <view key={key} aria-hidden className={cx(r.layer.base, !drawn && lines.length > 0 && r.layer.waiting)}>
      <vector paths={list} viewBox={frame} className={cx(r.drawing.base, r.drawing.moves)} />
    </view>
  );

  const reading = (row: ChartRow) => [
    ...shown
      .filter(series => Number.isFinite(numberOf(row, series.field)))
      .map(series => (series.label ? `${series.label} ${formatValue(numberOf(row, series.field))}` : formatValue(numberOf(row, series.field)))),
    ...(band && Number.isFinite(numberOf(row, band.low)) && Number.isFinite(numberOf(row, band.high))
      ? [`${band.label} ${formatValue(numberOf(row, band.low))}–${formatValue(numberOf(row, band.high))}`] : []),
  ].join(', ');

  const named = (row: ChartRow) => (reading(row) === '' ? textOf(row, x) : `${textOf(row, x)}: ${reading(row)}`);
  // With nothing chosen the scrubber rests on the latest row, which is what the line's end shows.
  const rest = chosen >= 0 ? chosen : data.length - 1;
  // One row has nowhere to scrub to; the readout still names it.
  const scrub = onSelect && facts.length > 0 && data.length > 1 ? (event: PointerEvent) => {
    const { phase } = event;
    if (phase === 'leave') return chosen >= 0 ? onSelect(null) : undefined;
    const current = chosen >= 0 ? keys[chosen] : null;
    if (phase === 'down') pressed(event, -1, current);
    // The page took the press for a scroll: what it chose on the way is put back.
    if (phase === 'cancel') return pressedKey() === current ? undefined : onSelect(pressedKey());
    // A finger chooses once it moves sideways or lifts, never as it lands.
    if ((phase === 'down' && event.pointer === 'touch') || (phase === 'move' && !scrubbing(event))) return undefined;
    const index = phase === 'increment' ? Math.min(data.length - 1, rest + 1)
      : phase === 'decrement' ? Math.max(0, rest - 1)
        : indexAt(event.x, event.width, data.length, 'points', edge);
    return index === chosen ? undefined : onSelect(keys[index]);
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
      legendPosition={legendPosition}
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
            {/* The labels, unseen, give the column its width: the moving ones are out of the flow. */}
            <view className={r.ghost.base}>{scale.map(tick => <text key={`tick-${tick.value}`} className={r.scaleLabel.base}>{tick.text}</text>)}</view>
            {rungs(tick => <text className={cx(r.scaleLabel.base, r.scaleLabel.level)}>{tick.text}</text>)}
          </view>
        )}
        <view box={plot} className={r.plot.base}>
          {scale
            // A hairline on each tick; the one the values stand on is firmer.
            ? <view aria-hidden className={cx(r.grid.base, r.inset.base, !isArea && r.inset.open)}>{rungs(tick => <view className={tick.value === range[0] ? r.rule.foot : r.rule.line} />)}</view>
            : <view aria-hidden className={cx(r.rule.bottom, !isArea && r.rule.open)} />}
          {zones.length > 0 && <vector aria-hidden paths={zones} viewBox={frame} className={cx(r.drawing.base, r.drawing.moves, r.drawing.over)} />}
          {referenceLines.length > 0 && <vector aria-hidden paths={referenceLines} viewBox={frame} className={cx(r.drawing.base, r.drawing.moves, r.drawing.over)} />}
          {layer(under, 'under')}
          {lines.length > 0 && <vector aria-hidden paths={drawIn(lines, drawn)} viewBox={frame} className={cx(r.drawing.base, r.drawing.moves)} />}
          {layer(over, 'over')}
          {marker.length > 0 && <vector aria-hidden paths={marker} viewBox={frame} className={cx(r.drawing.base, r.drawing.moves, r.drawing.over)} />}
          {refs.ghost}
          <ReferenceLabels texts={refs.texts} places={refs.places} shares={references.map(each => (scaleY(referenceTop(each), range) / box) * 100)} frame={cx(r.grid.base, r.inset.base, !isArea && r.inset.open)} />
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
              {data.map((row, index) => reading(row) !== '' && <text key={keys[index]} role="listitem">{named(row)}</text>)}
            </view>
          )}
          <ReferenceReading texts={refs.texts} />
        </view>
        {latest && (
          // It arrives with the dot that ends the line, and moves with it.
          <view aria-hidden className={cx(r.end.base, r.end.arrives, r.inset.base, !isArea && r.inset.open, !drawn && lines.length > 0 && r.layer.waiting)}>
            <view className={cx(r.mover.base, r.mover.end)} style={level(latest.value)}>
              <view className={r.endAnchor.base}><text className={cx(r.endLabel.base, r.endLabel.level)}>{latest.text}</text></view>
            </view>
          </view>
        )}
      </view>
    </Chart>
  );
}

/**
 * Before the first layout a solid line is hidden along its length, after it whole, so
 * `transition-[d]` draws it in. A dashed line has its own dashes and appears whole.
 */
function drawIn(paths: readonly VectorPath[], drawn: boolean): VectorPath[] {
  return paths.map(path => (path.strokeDasharray ? path : {
    ...path, pathLength: 1, strokeDasharray: [1], strokeDashoffset: drawn ? 0 : 1, ...(drawn ? {} : { strokeLinecap: 'butt' as const }),
  }));
}
