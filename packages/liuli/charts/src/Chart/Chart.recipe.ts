import type { ChartColor } from '../internal/data';
import type { ChartLegendVariant, ChartPlotSize } from './Chart';

export const chartParts = {
  root: 'view',
  header: 'view',
  title: 'text',
  description: 'text',
  summary: 'text',
  readout: 'view',
  readoutLabel: 'text',
  readoutValue: 'text',
  plot: 'view',
  state: 'view',
  stateIcon: 'vector',
  stateWords: 'view',
  stateAction: 'view',
  message: 'text',
  hint: 'text',
  skeleton: 'view',
  loadingBars: 'view',
  loadingLine: 'view',
  loadingRing: 'view',
  axis: 'view',
  ticks: 'view',
  tick: 'text',
  references: 'view',
  referenceMover: 'view',
  referenceAnchor: 'view',
  referenceLabel: 'text',
  referenceReading: 'view',
  legend: 'view',
  legendItem: 'button',
  legendSwatch: 'view',
  legendLine: 'vector',
  legendLabel: 'text',
  legendValue: 'text',
  legendDetail: 'text',
} as const;

/**
 * The frame every chart shares: a quiet heading, the plot at a fixed height, the legend in
 * footnote type. Colour appears only on marks and on the swatches that name them.
 */
export const chartRecipe = {
  root: {
    base: 'self-stretch gap-3',
  },
  header: {
    base: 'gap-0.5',
  },
  title: {
    base: 'text-body font-semibold text-text',
  },
  description: {
    base: 'text-footnote text-muted',
  },
  summary: {
    base: 'sr-only',
  },
  readout: {
    base: 'min-h-4.5 flex-row flex-wrap items-center gap-x-3',
  },
  ticks: {
    base: 'relative grow basis-0 min-w-0',
    // The labels a chart draws before the row is measured, over the same line.
    cover: 'absolute inset-0 flex-row',
  },
  tick: {
    base: 'text-caption text-muted tabular-nums whitespace-nowrap',
    // The longest label, unseen: it gives the row its height and the labels their width.
    ghost: 'self-start opacity-0',
    placed: 'absolute top-0',
    align: { start: 'text-start', center: 'text-center', end: 'text-end' },
  },
  /** Reference labels over the plot: a box as tall as the scale, so each label's place is its value's share. */
  references: {
    base: 'grow self-stretch relative',
    frame: 'absolute inset-0',
    ghost: 'absolute top-0 start-0 h-0 overflow-hidden opacity-0',
  },
  /** Moved down to its line on the marks' timing, so the label and the line arrive together. */
  referenceMover: {
    base: 'absolute inset-0 motion-safe:transition-transform duration-slow ease-standard',
  },
  referenceAnchor: {
    base: 'h-0 self-stretch',
  },
  /** At the end edge, a hair off its line; it never takes a press. */
  referenceLabel: {
    base: 'absolute end-0 text-caption text-muted tabular-nums whitespace-nowrap',
    measure: 'self-start text-caption text-muted tabular-nums whitespace-nowrap',
    above: 'bottom-0 pb-0.5',
    below: 'top-0 pt-0.5',
  },
  referenceReading: {
    base: 'sr-only',
  },
  readoutLabel: {
    base: 'text-footnote text-muted',
  },
  readoutValue: {
    base: 'text-footnote font-medium text-text tabular-nums',
  },
  plot: {
    base: 'self-stretch',
    size: { fit: '', xs: 'h-16', sm: 'h-28', md: 'h-44', lg: 'h-60' } satisfies Record<ChartPlotSize, string>,
    // A message taller than a small plot starts at its top; centred, it would be cut at both ends.
    state: 'items-center justify-center-safe',
  },
  /** Empty and error: a mark over the words, centred in the plot's own height. */
  state: {
    base: 'items-center gap-2 px-4',
    small: 'flex-row gap-1.5',
  },
  stateIcon: {
    empty: 'text-faint',
    error: 'text-danger',
  },
  stateWords: {
    base: 'items-center gap-0.5',
  },
  /** The retry button, centred under the message whatever its own alignment. */
  stateAction: {
    base: 'self-stretch flex-row justify-center pt-1',
  },
  message: {
    base: 'text-center text-footnote font-medium text-text',
  },
  hint: {
    base: 'text-center text-caption text-muted',
  },
  skeleton: {
    base: 'self-stretch grow',
  },
  /** Loading, as bars: six columns on the plot's floor. */
  loadingBars: {
    base: 'self-stretch grow flex-row items-end gap-3 px-2',
    bar: 'grow basis-0 rounded-b-none',
    height: { a: 'h-2/5', b: 'h-3/5', c: 'h-1/2', d: 'h-4/5', e: 'h-3/5', f: 'h-3/4' },
  },
  /** Loading, as a line: three faint rules and one smooth rise across the plot. Drawn, so it pulses where a skeleton glints. */
  loadingLine: {
    base: 'relative self-stretch grow motion-safe:animate-pulse',
    grid: 'absolute inset-0 justify-between py-1',
    rule: 'h-px bg-line',
    curve: 'absolute inset-0 size-full object-fill',
  },
  /** Loading, as a ring or a pie: the plot's height, as the chart draws it. */
  loadingRing: {
    base: 'self-stretch grow items-center justify-center motion-safe:animate-pulse',
    // A vector keeps its own size until given one: the plot's height.
    size: { fit: 'size-28', xs: 'size-16', sm: 'size-28', md: 'size-44', lg: 'size-60' } satisfies Record<ChartPlotSize, string>,
  },
  axis: {
    base: 'self-stretch -mt-1.5',
  },
  legend: {
    variant: { inline: 'flex-row flex-wrap gap-x-4 gap-y-1', list: 'self-stretch' } satisfies Record<ChartLegendVariant, string>,
  },
  legendItem: {
    variant: {
      inline: 'flex-row items-center gap-1.5 rounded-control text-start',
      list: 'min-h-9 flex-row items-center gap-2.5 rounded-control px-2 -mx-2 text-start',
    } satisfies Record<ChartLegendVariant, string>,
    pressable: 'transition-colors duration-fast ease-standard hover:bg-text/4 active:bg-text/8 focus-visible:bg-text/7',
    selected: 'bg-text/6',
  },
  legendSwatch: {
    base: 'text-footnote size-[0.76923em] shrink-0 rounded-[calc(var(--radius-sm)*0.5)]',
    color: { 1: 'bg-chart-1', 2: 'bg-chart-2', 3: 'bg-chart-3', 4: 'bg-chart-4', 5: 'bg-chart-5', 6: 'bg-chart-6', 7: 'bg-chart-7', 8: 'bg-chart-8' } satisfies Record<ChartColor, string>,
    other: 'bg-line-strong',
    off: 'bg-text/12',
  },
  legendLine: {
    base: 'h-1 w-4 shrink-0',
  },
  legendLabel: {
    base: 'text-footnote text-muted',
    list: 'min-w-0 grow basis-0 truncate text-text',
    off: 'text-faint line-through',
  },
  legendValue: {
    base: 'text-footnote font-medium text-text tabular-nums',
  },
  legendDetail: {
    base: 'min-w-[4ch] text-end text-footnote text-muted tabular-nums',
  },
} as const;
