import type { SwitchSize, SwitchVariant } from './Switch';

export const switchParts = {
  root: 'view',
  control: 'switch',
  description: 'text',
} as const;

/**
 * The switch primitive draws its label and, at its end, the track and thumb; the whole row is the
 * press target. Off is a quiet track, on is the accent; the thumb is a raised disc with a hairline
 * and contact shadow on both. Help text sits under it, outside the control, read as its description.
 */
export const switchRecipe = {
  root: {
    base: 'gap-0.5',
    variant: { standalone: '', row: 'self-stretch px-4 py-2.5 min-h-control-lg justify-center' } satisfies Record<SwitchVariant, string>,
  },
  control: {
    base: 'p-0 min-h-8 text-body text-text disabled:text-faint track:rounded-full track:bg-line-strong/45 checked:track:bg-accent thumb:rounded-full thumb:bg-surface-raised thumb:border thumb:border-text/10 thumb:shadow-contact focus-visible:thumb:ring-2 focus-visible:thumb:ring-focus disabled:checked:track:bg-accent/40 disabled:thumb:shadow-none',
    size: {
      sm: 'track:w-9 track:h-5 thumb:size-4',
      md: 'track:w-11 track:h-[26px] thumb:size-[22px]',
    } satisfies Record<SwitchSize, string>,
  },
  description: {
    base: 'text-footnote text-muted',
    /** Clear of the track above it. */
    size: { sm: 'pe-12', md: 'pe-14' } satisfies Record<SwitchSize, string>,
  },
} as const;
