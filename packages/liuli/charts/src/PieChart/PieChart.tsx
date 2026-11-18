import { cx, type VectorPath } from '@lingxia/view';
import { Chart, ChartLegend, type ChartColor, type ChartLegendPosition, type ChartRow, type ChartSize, type ChartSummarize } from '../Chart/Chart';
import { colorOf, keyOf, numberOf, plain, roleOf, seriesFacts, textOf } from '../internal/data';
import { box, circle, sector } from '../internal/geometry';
import { remember } from '../internal/memo';
import { pieChartRecipe as r } from './PieChart.recipe';

export type PieChartVariant = 'donut' | 'pie';

export interface PieChartProps {
  /** The rows, one per part, in the order drawn clockwise from the top. Parts without a positive number are left out. */
  rows: readonly ChartRow[];
  /** The field that names each part: "category". */
  nameField: string;
  /** The field that holds each part's amount. */
  valueField: string;
  /** The field that identifies a row across updates and in `value`. By default `nameField`. */
  keyField?: string;
  /** The chart's accessible name: "Spending by category". */
  label: string;
  title?: string;
  description?: string;
  /** Turns an amount into the text shown and read. By default the bare number. */
  formatValue?: (value: number) => string;
  /** Turns a part of the whole, 0 to 1, into text for the legend: "32%". Without it the legend shows amounts only. */
  formatShare?: (share: number) => string;
  /** Writes the summary a screen reader hears from the chart's facts; see `@liulidesign/charts/summary/en`. */
  summarize?: ChartSummarize;
  /** `donut` has a hole for the total; `pie` is solid. @default 'donut' */
  variant?: PieChartVariant;
  /** Names the total shown in a donut's hole: "Total". Without it the hole is empty until a part is chosen. */
  totalLabel?: string;
  /** Names one part that gathers every part after the first `maxParts`: "Other". Without it every part is drawn, up to 24. */
  otherLabel?: string;
  /** How many parts keep their own slice when `otherLabel` is set. @default 5 */
  maxParts?: number;
  /** The legend lists each part with its amount and share. @default 'bottom' */
  legend?: ChartLegendPosition;
  /** The legend's accessible name; by default the chart's `label`. */
  legendLabel?: string;
  /** The chosen part's key; its slice grows a little and keeps its colour, the rest step back. */
  value?: string | null;
  /** Makes each legend row a button; runs with its part's key, or `null` when the chosen one is pressed again. */
  onChange?: (key: string | null) => void;
  /** @default 'md' */
  size?: ChartSize;
  /** @default false */
  isLoading?: boolean;
  error?: string;
  /** What to say when no part has an amount. */
  emptyLabel?: string;
  /** A second, quieter line under `emptyLabel`: what to do next. */
  emptyHint?: string;
  /** With `error`: tries again. Pass `retryLabel` with it. */
  onRetry?: () => void;
  /** The words on the retry button: "Try again". */
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
export const otherKey = '__other';

/** Parts of one whole, as a ring with the total in its hole and the parts listed under it. */
export function PieChart({
  rows: data, nameField: name, valueField: value, keyField, label, title, description, formatValue: format, formatShare, summarize, variant = 'donut', totalLabel,
  otherLabel, maxParts = 5, legend = 'bottom', legendLabel, value: selectedKey, onChange: onSelect, size = 'md',
  isLoading = false, error, emptyLabel, emptyHint, onRetry, retryLabel, className, 'data-testid': testId,
}: PieChartProps) {
  const formatValue = format ?? plain;
  const listed = data
    .map((row, index) => ({ key: keyOf(row, keyField ?? name, index), name: textOf(row, name), amount: numberOf(row, value) }))
    .filter(part => part.amount > 0);
  const keep = otherLabel !== undefined ? Math.max(1, Math.min(maxParts, maxSlices - 1)) : maxSlices - 1;
  const rest = listed.slice(keep);
  // The remainder has no colour of its own: it is not one thing.
  const parts: { key: string; name: string; amount: number; color?: ChartColor }[] = [
    ...listed.slice(0, rest.length > 1 ? keep : listed.length).map((part, index) => ({ ...part, color: colorOf(index) })),
    ...(rest.length > 1 ? [{ key: otherKey, name: otherLabel ?? '', amount: rest.reduce((sum, part) => sum + part.amount, 0) }] : []),
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
      const role = part.color ? roleOf(part.color) : 'line-strong';
      const fill = chosen && chosen !== part ? `${role}/35` : role;
      const rim = chosen === part ? grown : outer;
      if (parts.length > 1) return { d: sector(inner, rim, from, to, gap), fill };
      return inner ? { d: circle((rim + inner) / 2), fill: 'none', stroke: fill, strokeWidth: rim - inner } : { d: circle(rim), fill };
    });
  });

  const facts = seriesFacts(parts.map(part => ({ name: part.name, amount: part.amount })), 'name', { field: 'amount', label: '' }, formatValue, true);
  const centre = chosen ? [chosen.name, formatValue(chosen.amount)] : totalLabel !== undefined ? [totalLabel, formatValue(total)] : undefined;
  return (
    <Chart
      label={label}
      title={title}
      description={description}
      summary={summarize?.({ kind: 'pie', count: parts.length, series: facts ? [facts] : [] })}
      size={size}
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
      legendPosition={legend}
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
        {paths.length > 0 && <vector aria-hidden paths={paths} viewBox={[0, 0, box, box]} className={cx(r.drawing.base, r.drawing.size[size])} />}
        {centre && (
          <view role="status" className={cx(r.centre.base, r.centre.variant[variant], r.centre.size[size])}>
            <text className={r.centreLabel.base}>{centre[0]}</text>
            <text className={r.centreValue.base}>{centre[1]}</text>
          </view>
        )}
      </view>
    </Chart>
  );
}
