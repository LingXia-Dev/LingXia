import type { ChartSize } from '../Chart/Chart';
import type { PieChartVariant } from './PieChart';

export const pieChartParts = {
  plot: 'view',
  drawing: 'vector',
  centre: 'view',
  centreLabel: 'text',
  centreValue: 'text',
} as const;

/**
 * The ring is as tall as the plot and centred; the figure for the whole, or for the chosen part,
 * sits in its hole. Parts are named in the legend under it, never on the ring.
 */
export const pieChartRecipe = {
  plot: {
    base: 'grow grid grid-cols-1 grid-rows-1 place-items-center',
  },
  drawing: {
    base: 'col-start-1 row-start-1',
    // A vector keeps its own size until given one: the ring is as tall as the plot.
    size: { xs: 'size-16', sm: 'size-28', md: 'size-44', lg: 'size-60' } satisfies Record<ChartSize, string>,
  },
  centre: {
    base: 'col-start-1 row-start-1 items-center',
    variant: { donut: '', pie: 'hidden' } satisfies Record<PieChartVariant, string>,
    // The hole's width at each plot height, less a margin; the smallest ring has no room for text.
    size: { xs: 'hidden', sm: 'max-w-14', md: 'max-w-[5.5rem]', lg: 'max-w-[7.5rem]' } satisfies Record<ChartSize, string>,
  },
  centreLabel: {
    base: 'max-w-full truncate text-caption text-muted',
  },
  centreValue: {
    base: 'text-callout font-semibold text-text tabular-nums',
  },
} as const;
