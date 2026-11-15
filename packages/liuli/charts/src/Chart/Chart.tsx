import { cx, type Child, type VectorPath } from '@lingxia/view';
import { Skeleton } from '@liulidesign/core/Skeleton';
import { roleOf, type ChartColor } from '../internal/data';
import { chartRecipe as r } from './Chart.recipe';

export type { ChartColor, ChartDatum, ChartFacts, ChartPointFact, ChartRow, ChartSeries, ChartSeriesFacts, ChartSummarize } from '../internal/data';

export type ChartSize = 'xs' | 'sm' | 'md' | 'lg';
/** `fit` is for a plot of your own that has its own height, such as a single bar. */
export type ChartPlotSize = ChartSize | 'fit';
export type ChartLegendVariant = 'inline' | 'list';
export type ChartLegendPosition = 'top' | 'bottom' | 'none';

export interface ChartProps {
  /** The chart's accessible name: what it shows ("Revenue by month"). Drawn only as `title`. */
  label: string;
  /** The heading above the plot. */
  title?: string;
  /** One line under the title: the period, the unit. */
  description?: string;
  /** The chart's finding in sentences, read after the name and not drawn. */
  summary?: string;
  /** The selected datum, shown above the plot; a `ChartReadout`. */
  readout?: Child;
  /** The plot: marks drawn to fill the box. */
  children?: Child;
  /** What sits under the plot and belongs to it: the category labels. */
  axis?: Child;
  /** A `ChartLegend`. */
  legend?: Child;
  /** @default 'bottom' */
  legendPosition?: ChartLegendPosition;
  /** The plot's height: 64, 112, 176 or 240, or `fit` to take the height of the plot's own content. `xs` is for a tile or a row, with nothing but the marks. @default 'md' */
  size?: ChartPlotSize;
  /** Stands a skeleton in for the plot; the chart is reported busy. @default false */
  isLoading?: boolean;
  /** Why the data could not be shown; drawn in place of the plot and announced. */
  error?: string;
  /** Shows `emptyLabel` in place of the plot. @default false */
  isEmpty?: boolean;
  /** What to say when there is nothing to draw: "No sales yet". */
  emptyLabel?: string;
  className?: string;
  /** On the chart. */
  'data-testid'?: string;
}

/** The frame a chart sits in: heading, summary, plot at a fixed height, legend, and the loading, empty and error states. */
export function Chart({
  label,
  title,
  description,
  summary,
  readout,
  children,
  axis,
  legend,
  legendPosition = 'bottom',
  size = 'md',
  isLoading = false,
  error,
  isEmpty = false,
  emptyLabel,
  className,
  'data-testid': testId,
}: ChartProps) {
  const isReady = !isLoading && error === undefined && !isEmpty;
  const hasLegend = legend !== undefined && isReady && legendPosition !== 'none';
  return (
    <view role="group" aria-label={label} aria-busy={isLoading || undefined} data-testid={testId} className={cx(r.root.base, className)}>
      {(title !== undefined || description !== undefined) && (
        <view className={r.header.base}>
          {/* A title that repeats the name is not read a second time. */}
          {title !== undefined && <text aria-hidden={title === label || undefined} className={r.title.base}>{title}</text>}
          {description !== undefined && <text className={r.description.base}>{description}</text>}
        </view>
      )}
      {summary !== undefined && isReady && <text className={r.summary.base}>{summary}</text>}
      {hasLegend && legendPosition === 'top' && legend}
      {isReady && readout}
      <view className={cx(r.plot.base, r.plot.size[size], !isReady && r.plot.state)}>
        {isLoading && <Skeleton shape="block" className={r.skeleton.base} />}
        {!isLoading && error !== undefined && <text role="alert" className={r.message.base}>{error}</text>}
        {!isLoading && error === undefined && isEmpty && emptyLabel !== undefined && <text className={r.message.base}>{emptyLabel}</text>}
        {isReady && children}
      </view>
      {isReady && axis !== undefined && <view aria-hidden className={r.axis.base}>{axis}</view>}
      {hasLegend && legendPosition === 'bottom' && legend}
    </view>
  );
}

export interface ChartLegendItem {
  /** The series field or the row key this entry stands for. */
  key: string;
  label: string;
  /** Its mark's colour; leave it out for a merged remainder, drawn in a neutral tone. */
  color?: ChartColor;
  /** The mark's dash pattern, so the entry matches a line that is told apart without colour. */
  dash?: readonly number[];
  /** A figure after the label, as the app formats it. Lists only. */
  value?: string;
  /** A second, quieter figure at the end: a share. Lists only. */
  detail?: string;
  /** The entry's mark is not drawn at present. @default false */
  isOff?: boolean;
}

export interface ChartLegendProps {
  /** The list's accessible name: "Series", "Parts". */
  label: string;
  items: readonly ChartLegendItem[];
  /** `inline` wraps entries on a line; `list` gives each entry a row with its figures. @default 'inline' */
  variant?: ChartLegendVariant;
  /** `line` draws each swatch as a stroke with the entry's dash. @default 'square' */
  mark?: 'square' | 'line';
  /** The entry shown as chosen. Lists only. */
  selectedKey?: string | null;
  /** Makes each entry a button and runs with its key. */
  onItemPress?: (key: string) => void;
  /** What a pressed entry means to assistive tech: `selected` picks one, `shown` toggles a series. @default 'selected' */
  pressedState?: 'selected' | 'shown';
  className?: string;
  'data-testid'?: string;
}

const lineBox = [0, 0, 16, 4] as const;

/** The names of a chart's series or parts, each beside a swatch of its mark, in Liuli type. */
export function ChartLegend({
  label, items, variant = 'inline', mark = 'square', selectedKey, onItemPress, pressedState = 'selected', className, 'data-testid': testId,
}: ChartLegendProps) {
  return (
    <view role="list" aria-label={label} data-testid={testId} className={cx(r.legend.variant[variant], className)}>
      {items.map(item => {
        const isOff = item.isOff === true;
        const line: VectorPath[] = [{
          d: 'M1 2H15', fill: 'none', stroke: item.color && !isOff ? roleOf(item.color) : 'text-faint', strokeWidth: 2, strokeLinecap: 'round',
          ...(item.dash?.length ? { strokeDasharray: item.dash } : {}),
        }];
        const content = [
          mark === 'line'
            ? <vector key="mark" aria-hidden paths={line} viewBox={lineBox} className={r.legendLine.base} />
            : <view key="mark" aria-hidden className={cx(r.legendSwatch.base, isOff ? r.legendSwatch.off : item.color ? r.legendSwatch.color[item.color] : r.legendSwatch.other)} />,
          <text key="label" className={cx(r.legendLabel.base, variant === 'list' && r.legendLabel.list, isOff && r.legendLabel.off)}>{item.label}</text>,
          variant === 'list' && item.value !== undefined && <text key="value" className={r.legendValue.base}>{item.value}</text>,
          variant === 'list' && item.detail !== undefined && <text key="detail" className={r.legendDetail.base}>{item.detail}</text>,
        ];
        const isSelected = selectedKey === item.key;
        if (!onItemPress) return <view key={item.key} role="listitem" className={r.legendItem.variant[variant]}>{content}</view>;
        return (
          <view key={item.key} role="listitem">
            <button
              aria-pressed={pressedState === 'shown' ? !isOff : isSelected}
              onClick={() => onItemPress(item.key)}
              data-testid={testId === undefined ? undefined : `${testId}-${item.key}`}
              className={cx(r.legendItem.variant[variant], r.legendItem.pressable, isSelected && r.legendItem.selected)}
            >
              {content}
            </button>
          </view>
        );
      })}
    </view>
  );
}

export interface ChartReadoutValue {
  /** The series name, when there is more than one. */
  label?: string;
  /** The figure, as the app formats it. */
  text: string;
  color?: ChartColor;
}

export interface ChartReadoutProps {
  /** The datum's category: "Wednesday". Leave it out to keep the line's height with nothing in it. */
  label?: string;
  /** @default [] */
  values?: readonly ChartReadoutValue[];
  'data-testid'?: string;
}

/** The chosen datum in words and figures, on a line of its own that keeps its height when nothing is chosen. */
export function ChartReadout({ label, values = [], 'data-testid': testId }: ChartReadoutProps) {
  return (
    <view role="status" data-testid={testId} className={r.readout.base}>
      {label !== undefined && <text className={r.readoutLabel.base}>{label}</text>}
      {label !== undefined && values.map((value, index) => (
        <view key={String(index)} className={r.legendItem.variant.inline}>
          {value.color !== undefined && <view aria-hidden className={cx(r.legendSwatch.base, r.legendSwatch.color[value.color])} />}
          {value.label !== undefined && <text className={r.readoutLabel.base}>{value.label}</text>}
          <text className={r.readoutValue.base}>{value.text}</text>
        </view>
      ))}
    </view>
  );
}
