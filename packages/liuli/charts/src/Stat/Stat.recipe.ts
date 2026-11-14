import type { StatSize, StatTone } from './Stat';

export const statParts = {
  root: 'view',
  label: 'text',
  value: 'text',
  change: 'view',
  delta: 'view',
  deltaGlyph: 'vector',
  deltaText: 'text',
  deltaSpoken: 'text',
  description: 'text',
  chart: 'view',
  skeleton: 'view',
} as const;

/**
 * Label, figure, then the change and its note on one line, then the chart. The figure is the only
 * strong thing; the change takes its tone's colour, an arrow and footnote size.
 */
export const statRecipe = {
  root: {
    base: 'min-w-0 gap-0.5',
  },
  label: {
    base: 'text-footnote font-medium text-muted',
  },
  value: {
    base: 'font-semibold tracking-tight text-text tabular-nums',
    size: { sm: 'text-title', md: 'text-display' } satisfies Record<StatSize, string>,
  },
  change: {
    base: 'flex-row flex-wrap items-center gap-x-1.5',
  },
  delta: {
    base: 'flex-row items-center gap-0.5',
    tone: { neutral: 'text-muted', positive: 'text-success', negative: 'text-danger' } satisfies Record<StatTone, string>,
  },
  deltaGlyph: {
    base: 'size-3',
  },
  deltaText: {
    base: 'text-footnote font-medium tabular-nums',
  },
  deltaSpoken: {
    base: 'sr-only',
  },
  description: {
    base: 'text-footnote text-muted',
  },
  chart: {
    base: 'self-stretch pt-1.5',
  },
  skeleton: {
    size: { sm: 'my-1 h-5 w-20', md: 'my-1 h-[1.625rem] w-24' } satisfies Record<StatSize, string>,
  },
} as const;
