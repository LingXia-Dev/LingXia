import type { TimerSize } from './Timer';

export const timerParts = { root: 'text' } as const;

/** Digits that do not shift as they change. */
export const timerRecipe = {
  root: {
    base: 'tabular-nums text-text',
    size: { sm: 'text-footnote', md: 'text-body font-medium', lg: 'text-display font-semibold tracking-tight' } satisfies Record<TimerSize, string>,
    done: 'text-muted',
  },
} as const;
