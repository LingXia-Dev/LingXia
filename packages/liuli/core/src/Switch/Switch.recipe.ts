import type { SwitchVariant } from './Switch';

export const switchParts = {
  root: 'view',
  control: 'switch',
  description: 'text',
} as const;

/**
 * The switch primitive draws its label and, at its end, the track and thumb; the whole row is the
 * press target. Help text sits under it, outside the control, and is read as its description.
 */
export const switchRecipe = {
  root: {
    base: 'gap-0.5',
    variant: { standalone: '', row: 'self-stretch px-4 py-2.5 min-h-control-lg justify-center' } satisfies Record<SwitchVariant, string>,
  },
  control: {
    base: 'p-0 min-h-8 text-body text-text disabled:text-faint',
  },
  description: {
    base: 'text-footnote text-muted pe-16',
  },
} as const;
