import { cx, useBox, useLocal, useView, type ColorAlpha, type ColorRole, type PointerEvent, type VectorPath, type Local } from '@lingxia/view';
import { Chart, ChartLegend, type ChartColor, type ChartField, type ChartLegendPosition, type ChartRecord, type ChartRow, type ChartSize, type ChartSummarize } from '../Chart/Chart';
import { liuliText } from '@liulidesign/core/internal/text';
import { colorOf, keysOf, numberOf, numberIn, roleOf, seriesFacts, textOf } from '../internal/data';
import { box, circle, sector } from '../internal/geometry';
import { remember } from '../internal/memo';
import { pieChartRecipe as r } from './PieChart.recipe';

export type PieChartVariant = 'donut' | 'pie';

/** `Row` is the app's own record type, so the fields named are its own and a typo fails to compile. */
export interface PieChartProps<Row extends ChartRecord<Row> = ChartRow> {
  /** The rows, one per part, in the order drawn clockwise from the top. Parts without a positive number are left out. */
  rows: readonly Row[];
  /** The field that names each part: "category". */
  nameField: ChartField<Row>;
  /** The field that holds each part's amount. */
  valueField: ChartField<Row>;
  /** The field that identifies a row across updates and in `value`. By default `nameField`. */
  keyField?: ChartField<Row>;
  /** The chart's accessible name: "Spending by category". */
  label: string;
  title?: string;
  description?: string;
  /** Turns an amount into the text shown and read. By default the number as the page's language writes it, grouped (`Intl.NumberFormat`). */
  formatValue?: (value: number) => string;
  /** Turns a part of the whole, 0 to 1, into text for the legend: "32%". Without it the legend shows amounts only. */
  formatShare?: (share: number) => string;
  /** Writes the summary a screen reader hears from the chart's facts; see `@liulidesign/charts/summary/en`. */
  summarize?: ChartSummarize;
  /** `donut` has a hole for the total; `pie` is solid. @default 'donut' */
  variant?: PieChartVariant;
  /** Names the total shown in a donut's hole. `null` leaves the hole empty until a part is chosen. A donut's defaults to "Total" in the page's language; a solid pie shows no total without one. */
  totalLabel?: string | null;
  /** Names one part that gathers every part after the first `maxParts`: "Other". Without it every part is drawn up to 24, and any past those are one part named "Other" in the page's language. */
  otherLabel?: string;
  /** How many parts keep their own slice when `otherLabel` is set. @default 5 */
  maxParts?: number;
  /** The legend lists each part with its amount and share. @default 'bottom' */
  legendPosition?: ChartLegendPosition;
  /** The legend's accessible name; by default the chart's `label`. */
  legendLabel?: string;
  /** The chosen part's key; its slice grows a little and keeps its colour, the rest step back. */
  value?: string | null;
  /** Makes each legend row a button; runs with its part's key, or `null` when the chosen one is pressed again. */
  onChange?: (key: string | null) => void;
  /** Where the choice starts when the chart keeps it itself: give this instead of `value`; `onChange` still hears each choice. */
  defaultValue?: string | null;
  /** A View-local cell holding the chosen key, read and written instead of `value` with `onChange`; `onChange` still hears each choice. */
  bind?: Local<string | null>;
  /** @default 'md' */
  size?: ChartSize;
  /** @default false */
  isLoading?: boolean;
  error?: string;
  /** What to say when no part has an amount. Defaults to "No data" in the page's language. */
  emptyLabel?: string;
  /** A second, quieter line under `emptyLabel`: what to do next. */
  emptyHint?: string;
  /** With `error`: tries again. */
  onRetry?: () => void;
  /** The words on the retry button. Defaults to "Try again" in the page's language. */
  retryLabel?: string;
  className?: string;
  /** On the chart; a legend row's button is `<id>-legend-<key>`. */
  'data-testid'?: string;
}

// In the drawing's 100-unit box: the ring leaves room for a chosen slice to grow.
const outer = 46;
const grown = 50;
const hole = 29;
const gap = 1.6;
// One path per slice, and a vector holds 32.
const maxSlices = 24;
// How far past the rim, and into the hole, a tap still counts: a finger is wider than the ring's edge.
const reachPast = 4;
const reachIn = 6;
export const otherKey = '__other';

/** Parts of one whole, as a ring with the total in its hole and the parts listed under it. */
export function PieChart<Row extends ChartRecord<Row>>({
  rows, nameField: name, valueField: value, keyField, label, title, description, formatValue: format, formatShare, summarize, variant = 'donut', totalLabel: ownTotalLabel,
  otherLabel, maxParts = 5, legendPosition = 'bottom', legendLabel, value: given, onChange, defaultValue, bind, size = 'md',
  isLoading = false, error, emptyLabel, emptyHint, onRetry, retryLabel, className, 'data-testid': testId,
}: PieChartProps<Row>) {
  // Without `value`, a `defaultValue` makes the choice the chart's own.
  const own = useLocal<string | null>('chosen', defaultValue ?? null);
  const isOwn = given === undefined && defaultValue !== undefined;
  const selectedKey = bind ? bind.value : isOwn ? own.value : given;
  const onSelect = bind ? (key: string | null) => { bind.set(key); onChange?.(key); } : isOwn ? (key: string | null) => { own.set(key); onChange?.(key); } : onChange;
  // Only the fields named are read, and each of those holds a datum.
  const data = rows as readonly ChartRow[];
  // Without the app's own, values are written as the page's language writes numbers.
  const formatValue = format ?? numberIn(useView().env.language);
  const keys = keysOf(data, keyField ?? name, [otherKey]);
  const listed = data
    .map((row, index) => ({ key: keys[index], name: textOf(row, name), amount: numberOf(row, value) }))
    .filter(part => part.amount > 0);
  const parted = Number.isFinite(maxParts) ? Math.floor(maxParts) : 5;
  const keep = otherLabel !== undefined ? Math.max(1, Math.min(parted, maxSlices - 1)) : maxSlices - 1;
  const rest = listed.slice(keep);
  // The remainder has no colour of its own: it is not one thing.
  const parts: { key: string; name: string; amount: number; color?: ChartColor }[] = [
    ...listed.slice(0, rest.length > 1 ? keep : listed.length).map((part, index) => ({ ...part, color: colorOf(index) })),
    ...(rest.length > 1 ? [{ key: otherKey, name: otherLabel ?? liuliText('@liuli.chart.other'), amount: rest.reduce((sum, part) => sum + part.amount, 0) }] : []),
  ];
  const total = parts.reduce((sum, part) => sum + part.amount, 0);
  const chosen = parts.find(part => part.key === selectedKey);
  const inner = variant === 'donut' ? hole : 0;

  // The slices depend on the data, the encoding and the chosen part, never on the theme: colours are role names.
  const paths = remember(data, 'slices', JSON.stringify([name, value, keyField, variant, otherLabel, maxParts, selectedKey]), () => {
    let turned = 0;
    return parts.map((part): VectorPath => {
      const [from, to] = [turned, turned + (part.amount / total) * 2 * Math.PI];
      turned = to;
      const role: ColorRole = part.color ? roleOf(part.color) : 'line-strong';
      const fill: ColorRole | ColorAlpha = chosen && chosen !== part ? `${role}/35` : role;
      const rim = chosen === part ? grown : outer;
      if (parts.length > 1) return { d: sector(inner, rim, from, to, gap), fill };
      return inner ? { d: circle((rim + inner) / 2), fill: 'none', stroke: fill, strokeWidth: rim - inner } : { d: circle(rim), fill };
    });
  });

  const facts = seriesFacts(parts.map(part => ({ name: part.name, amount: part.amount })), 'name', { field: 'amount', label: '' }, formatValue, true);
  const totalLabel = ownTotalLabel !== undefined ? ownTotalLabel : variant === 'donut' ? liuliText('@liuli.chart.total') : null;
  const centre = chosen ? [chosen.name, formatValue(chosen.amount)] : totalLabel !== null ? [totalLabel, formatValue(total)] : undefined;
  const ring = useBox('ring');
  // A tap on a slice chooses it, as its legend entry does; the legend stays the way in for a keyboard and a screen reader.
  const partAt = (event: PointerEvent) => {
    const side = Math.min(event.width, event.height);
    if (!(side > 0) || !total) return undefined;
    const [dx, dy] = [(event.x - event.width / 2) * (box / side), (event.y - event.height / 2) * (box / side)];
    const radius = Math.hypot(dx, dy);
    if (radius > grown + reachPast || radius < (inner ? inner - reachIn : 0)) return undefined;
    // Clockwise from the top, as the slices are laid.
    const turn = (Math.atan2(dy, dx) + Math.PI / 2 + 2 * Math.PI) % (2 * Math.PI);
    let turned = 0;
    return parts.find(part => (turned += (part.amount / total) * 2 * Math.PI) > turn) ?? parts[parts.length - 1];
  };
  const ringPointer = onSelect ? {
    // A drag up or down stays with the page.
    pointerAxis: 'x' as const,
    onPointer: (event: PointerEvent) => {
      if (event.phase !== 'up') return;
      const part = partAt(event);
      if (part) onSelect(part.key === selectedKey ? null : part.key);
    },
    'data-testid': testId === undefined ? undefined : `${testId}-ring`,
  } : {};
  const centreBox = useBox('centre');
  const figure = useBox('figure');
  // Words whose corners would cross the ring, as at large text, go under it instead.
  const [across, high, wide] = [ring.value?.width ?? 0, centreBox.value?.height ?? 0, figure.value?.width ?? 0];
  const isUnder = variant === 'donut' && across > 0 && wide > 0 && Math.hypot(wide, high) > (across * 2 * hole) / box;
  const words = centre && (
    <view box={centreBox} role="status" className={cx(r.centre.base, isUnder ? r.centre.under : cx(r.centre.variant[variant], r.centre.size[size]))}>
      <text className={r.centreLabel.base}>{centre[0]}</text>
      <text className={r.centreValue.base}>{centre[1]}</text>
    </view>
  );
  return (
    <Chart
      label={label}
      title={title}
      description={description}
      summary={summarize?.({ kind: 'pie', count: parts.length, series: facts ? [facts] : [] })}
      // Words under the ring add their height to the plot.
      size={isUnder && !isLoading && error === undefined ? 'fit' : size}
      isLoading={isLoading}
      plotShape={variant === 'donut' ? 'ring' : 'disc'}
      error={error}
      onRetry={onRetry}
      retryLabel={retryLabel}
      emptyHint={emptyHint}
      isEmpty={!parts.length}
      emptyLabel={emptyLabel}
      className={className}
      data-testid={testId}
      legendPosition={legendPosition}
      legend={(
        <ChartLegend
          label={legendLabel ?? label}
          variant="list"
          items={parts.map(part => ({
            key: part.key, label: part.name, color: part.color, value: formatValue(part.amount), detail: formatShare?.(part.amount / total),
          }))}
          value={selectedKey}
          onItemPress={onSelect && (key => onSelect(key === selectedKey ? null : key))}
          data-testid={testId === undefined ? undefined : `${testId}-legend`}
        />
      )}
    >
      <view className={r.plot.base}>
        {paths.length > 0 && <vector box={ring} aria-hidden {...ringPointer} paths={paths} viewBox={[0, 0, box, box]} className={cx(r.drawing.base, r.drawing.size[size])} />}
        {centre && variant === 'donut' && size !== 'xs' && <text aria-hidden box={figure} className={cx(r.centreValue.base, r.centreValue.ghost)}>{centre[1]}</text>}
        {!isUnder && words}
      </view>
      {isUnder && words}
      {/* Without a legend the parts are still read, each with its amount. */}
      {legendPosition === 'none' && (
        <view role="list" aria-label={legendLabel ?? label} className={r.reading.base}>
          {parts.map(part => <text key={part.key} role="listitem">{`${part.name}: ${formatValue(part.amount)}${formatShare ? `, ${formatShare(part.amount / total)}` : ''}`}</text>)}
        </view>
      )}
    </Chart>
  );
}
