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
  message: 'text',
  skeleton: 'view',
  axis: 'view',
  ticks: 'view',
  tick: 'text',
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
    base: 'min-h-[1.125rem] flex-row flex-wrap items-center gap-x-3',
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
  message: {
    base: 'px-4 text-center text-footnote text-muted',
  },
  skeleton: {
    base: 'self-stretch grow',
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
    base: 'size-2.5 shrink-0 rounded-[3px]',
    color: { 1: 'bg-chart-1', 2: 'bg-chart-2', 3: 'bg-chart-3', 4: 'bg-chart-4', 5: 'bg-chart-5', 6: 'bg-hue-3', 7: 'bg-hue-5', 8: 'bg-hue-4' } satisfies Record<ChartColor, string>,
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
