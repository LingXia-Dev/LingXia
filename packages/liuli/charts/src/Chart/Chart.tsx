import { cx, type Child, type VectorPath } from '@lingxia/view';
import { Button } from '@liulidesign/core/Button';
import { Icon } from '@liulidesign/core/Icon';
import { Skeleton } from '@liulidesign/core/Skeleton';
import { roleOf, type ChartColor } from '../internal/data';
import { circle } from '../internal/geometry';
import { chartRecipe as r } from './Chart.recipe';

export type { ChartColor, ChartDatum, ChartFacts, ChartField, ChartPointFact, ChartRecord, ChartReference, ChartRow, ChartSeries, ChartSeriesFacts, ChartSummarize } from '../internal/data';

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
  /** What the plot is: the loading skeleton takes its outline and the empty state its mark, so a chart with nothing to draw still looks like itself. @default 'block' */
  plotShape?: ChartPlotShape;
  /** Why the data could not be shown; drawn in place of the plot with an error mark, and announced. */
  error?: string;
  /** With `error`: tries again. Pass `retryLabel` with it. */
  onRetry?: () => void;
  /** The words on the retry button: "Try again". */
  retryLabel?: string;
  /** Shows `emptyLabel` in place of the plot. @default false */
  isEmpty?: boolean;
  /** What to say when there is nothing to draw: "No sales yet". */
  emptyLabel?: string;
  /** A second, quieter line under `emptyLabel`: what to do next. */
  emptyHint?: string;
  className?: string;
  /** On the chart. */
  'data-testid'?: string;
}

export type ChartPlotShape = 'block' | 'bars' | 'line' | 'ring' | 'disc';

const barHeights = ['a', 'b', 'c', 'd', 'e', 'f'] as const;
// A line chart at rest: a smooth rise with a faint area under it, on a 240 by 80 box that stretches to the plot.
const curve = 'M0 62C40 60 56 50 84 48S128 52 150 38S196 20 240 16';
const loadingLine: VectorPath[] = [
  { d: `${curve}V80H0Z`, fill: 'text/5' },
  { d: curve, fill: 'none', stroke: 'text/16', strokeWidth: 2.5, strokeLinecap: 'round', vectorEffect: 'non-scaling-stroke' },
];
const loadingBox = [0, 0, 240, 80] as const;
// The donut's ring and the pie's disc, on the charts' 100 box.
const loadingRing: VectorPath[] = [{ d: circle(37.5), fill: 'none', stroke: 'text/10', strokeWidth: 17 }];
const loadingDisc: VectorPath[] = [{ d: circle(46), fill: 'text/10' }];
const pieBox = [0, 0, 100, 100] as const;
// The empty state's mark by what the plot is, drawn like the icon set's own chart mark.
const axes = 'M3 3v16a2 2 0 0 0 2 2h16';
const marks = {
  line: { paths: [axes, 'M7 15l4-4 3 2 5-7'] },
  ring: { paths: ['M12 3a9 9 0 1 0 0 18a9 9 0 1 0 0-18', 'M12 8.5a3.5 3.5 0 1 0 0 7a3.5 3.5 0 1 0 0-7'] },
  disc: { paths: ['M12 3a9 9 0 1 0 0 18a9 9 0 1 0 0-18', 'M12 3v9h9'] },
} as const;

/** The plot's stand-in while loading: bars with a skeleton's glint, or a line or a ring drawn faintly and pulsing. */
function LoadingPlot({ shape, size }: { shape: ChartPlotShape; size: ChartPlotSize }) {
  if (shape === 'bars')
    return <view aria-hidden className={r.loadingBars.base}>{barHeights.map(step => <Skeleton key={step} shape="block" className={cx(r.loadingBars.bar, r.loadingBars.height[step])} />)}</view>;
  if (shape === 'line')
    return (
      <view aria-hidden className={r.loadingLine.base}>
        <view className={r.loadingLine.grid}>
          <view className={r.loadingLine.rule} />
          <view className={r.loadingLine.rule} />
          <view className={r.loadingLine.rule} />
        </view>
        <vector paths={loadingLine} viewBox={loadingBox} className={r.loadingLine.curve} />
      </view>
    );
  if (shape === 'ring' || shape === 'disc')
    return <view aria-hidden className={r.loadingRing.base}><vector paths={shape === 'ring' ? loadingRing : loadingDisc} viewBox={pieBox} className={r.loadingRing.size[size]} /></view>;
  return <Skeleton shape="block" className={r.skeleton.base} />;
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
  plotShape = 'block',
  error,
  onRetry,
  retryLabel,
  isEmpty = false,
  emptyLabel,
  emptyHint,
  className,
  'data-testid': testId,
}: ChartProps) {
  const isReady = !isLoading && error === undefined && !isEmpty;
  // The shortest plot has room for one line: the mark sits beside the words, and nothing else is drawn.
  const isSmall = size === 'xs';
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
      {/* Drawn data keeps its physical order in either direction: the marks, figures, slots and axis run left to right. */}
      <view dir={isReady ? 'ltr' : undefined} className={cx(r.plot.base, r.plot.size[size], !isReady && r.plot.state)}>
        {isLoading && <LoadingPlot shape={plotShape} size={size} />}
        {!isLoading && error !== undefined && (
          // A mark and a way on, so a failure reads as a state the chart is in, not as nothing drawn.
          <view className={cx(r.state.base, isSmall && r.state.small)}>
            <Icon name="error" size={isSmall ? 'sm' : 'md'} className={r.stateIcon.error} />
            <text role="alert" className={r.message.base}>{error}</text>
            {!isSmall && onRetry !== undefined && retryLabel !== undefined && (
              <view className={r.stateAction.base}>
                <Button label={retryLabel} size="sm" width="content" onClick={onRetry} data-testid={testId && `${testId}-retry`} />
              </view>
            )}
          </view>
        )}
        {!isLoading && error === undefined && isEmpty && emptyLabel !== undefined && (
          <view className={cx(r.state.base, isSmall && r.state.small)}>
            {plotShape === 'line' || plotShape === 'ring' || plotShape === 'disc'
              ? <Icon icon={marks[plotShape]} size={isSmall ? 'sm' : 'md'} className={r.stateIcon.empty} />
              : <Icon name="chart" size={isSmall ? 'sm' : 'md'} className={r.stateIcon.empty} />}
            <view className={r.stateWords.base}>
              <text className={r.message.base}>{emptyLabel}</text>
              {!isSmall && emptyHint !== undefined && <text className={r.hint.base}>{emptyHint}</text>}
            </view>
          </view>
        )}
        {isReady && children}
      </view>
      {isReady && axis !== undefined && <view aria-hidden dir="ltr" className={r.axis.base}>{axis}</view>}
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
  value?: string | null;
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
  label, items, variant = 'inline', mark = 'square', value: selectedKey, onItemPress, pressedState = 'selected', className, 'data-testid': testId,
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
