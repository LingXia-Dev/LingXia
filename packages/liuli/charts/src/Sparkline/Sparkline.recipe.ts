import type { SparklineSize } from './Sparkline';

export const sparklineParts = {
  root: 'view',
  plot: 'vector',
} as const;

/**
 * The drawing stretches to its box and keeps its stroke width. A vector keeps its own size until
 * given one, so it takes the box's width outright and its height by growing.
 */
export const sparklineRecipe = {
  root: {
    base: 'self-stretch',
    size: { sm: 'h-6', md: 'h-8', lg: 'h-12' } satisfies Record<SparklineSize, string>,
  },
  plot: {
    base: 'w-full grow object-fill',
  },
} as const;
