import type { ChartColor } from '../Chart/Chart';

export const barChartParts = {
  scale: 'text',
  plot: 'view',
  slot: 'button',
  column: 'view',
  space: 'view',
  valueSlot: 'view',
  value: 'text',
  bar: 'view',
  baseline: 'view',
  labels: 'view',
  label: 'text',
} as const;

/**
 * Bars are boxes laid out by flex, so their corners and gaps are true points at any width. Each
 * category is a full-height slot, which is also its press target; its bars take about two thirds
 * of it, so the gap between categories is always wider than the gap inside one. The fill is the
 * series colour a little under full strength, which sits in a light card without shouting.
 */
export const barChartRecipe = {
  scale: {
    base: 'pb-1 text-footnote text-muted tabular-nums',
  },
  plot: {
    base: 'relative grow flex-row items-stretch',
    ruled: 'border-t border-text/8',
    floored: 'border-b border-text/8',
  },
  slot: {
    base: 'grow basis-0 min-w-0 flex-row items-stretch gap-0.5 rounded-[4px]',
    pressable: 'hover:bg-text/4 focus-visible:bg-text/7',
  },
  column: {
    base: 'grow basis-0 max-w-9 items-center',
  },
  space: {
    base: 'basis-0',
  },
  valueSlot: {
    base: 'h-5 shrink-0 items-center justify-center',
  },
  value: {
    base: 'text-footnote text-muted tabular-nums whitespace-nowrap',
  },
  bar: {
    base: 'self-stretch basis-0 transition-layout duration-200 ease-out',
    up: 'rounded-t-[4px]',
    down: 'rounded-b-[4px]',
    color: { 1: 'bg-chart-1/85', 2: 'bg-chart-2/85', 3: 'bg-chart-3/85', 4: 'bg-chart-4/85', 5: 'bg-chart-5/85' } satisfies Record<ChartColor, string>,
    dimmed: { 1: 'bg-chart-1/30', 2: 'bg-chart-2/30', 3: 'bg-chart-3/30', 4: 'bg-chart-4/30', 5: 'bg-chart-5/30' } satisfies Record<ChartColor, string>,
  },
  baseline: {
    base: 'absolute inset-0',
    above: 'basis-0 border-b border-text/20',
  },
  labels: {
    all: 'flex-row pt-3',
    ends: 'flex-row justify-between pt-3',
  },
  label: {
    base: 'text-caption text-muted tabular-nums',
    cell: 'grow basis-0 min-w-0 truncate text-center',
  },
} as const;
