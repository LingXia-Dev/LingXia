import type { Hue } from '../hue';
import type { StatusDotSize, StatusDotTone } from './StatusDot';

export const statusDotParts = {
  root: 'view',
  dot: 'view',
  label: 'text',
} as const;

/** A dot in a status mark colour with the status in words beside it; neutral is a hollow ring, so it differs by shape too. */
export const statusDotRecipe = {
  root: {
    base: 'flex-row items-center shrink-0',
    size: { sm: 'gap-1.5', md: 'gap-2' } satisfies Record<StatusDotSize, string>,
  },
  dot: {
    base: 'shrink-0 rounded-full',
    size: { sm: 'size-2', md: 'size-2.5' } satisfies Record<StatusDotSize, string>,
    tone: {
      success: 'bg-success-mark',
      warning: 'bg-warning-mark',
      danger: 'bg-danger-mark',
      info: 'bg-info-mark',
      accent: 'bg-accent',
      neutral: 'border-2 border-line-strong',
    } satisfies Record<StatusDotTone, string>,
    hue: { 1: 'bg-hue-1', 2: 'bg-hue-2', 3: 'bg-hue-3', 4: 'bg-hue-4', 5: 'bg-hue-5', 6: 'bg-hue-6', 7: 'bg-hue-7', 8: 'bg-hue-8' } satisfies Record<Hue, string>,
  },
  label: {
    base: 'text-text',
    size: { sm: 'text-footnote', md: 'text-body' } satisfies Record<StatusDotSize, string>,
  },
} as const;
