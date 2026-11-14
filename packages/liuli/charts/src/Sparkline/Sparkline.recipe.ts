import type { SparklineSize, SparklineVariant } from './Sparkline';

export const sparklineParts = {
  root: 'view',
  plot: 'vector',
} as const;

/**
 * The drawing stretches to its box and keeps its stroke width. A vector keeps its own size until
 * given one, so it takes the box's width outright and its height by growing. A line is inset by half its dot, so
 * neither the stroke nor the end dot is cut at an edge; bars and the foot of an area reach the edges.
 */
export const sparklineRecipe = {
  root: {
    base: 'self-stretch',
    size: { sm: 'h-6', md: 'h-8', lg: 'h-12' } satisfies Record<SparklineSize, string>,
  },
  plot: {
    base: 'w-full grow object-fill',
    variant: { line: 'p-[3px]', area: 'px-[3px] pt-[3px]', bar: '' } satisfies Record<SparklineVariant, string>,
  },
} as const;
