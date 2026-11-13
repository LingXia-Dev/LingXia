import type { SwitchSize, SwitchVariant } from './Switch';

export const switchParts = {
  root: 'view',
  control: 'switch',
  text: 'view',
  label: 'text',
  description: 'text',
} as const;

/**
 * The switch primitive holds the label and help text as its content and draws the track and thumb
 * at its end, so the whole block is the press target. Off is a quiet track, on is the accent; the
 * thumb is a raised disc with a hairline and contact shadow on both.
 */
export const switchRecipe = {
  root: {
    base: '',
    variant: { standalone: '', row: 'self-stretch px-4 py-2.5 min-h-control-lg justify-center' } satisfies Record<SwitchVariant, string>,
  },
  control: {
    base: 'min-h-8 track:rounded-full track:bg-line-strong/45 checked:track:bg-accent thumb:rounded-full thumb:bg-surface-raised thumb:border thumb:border-text/10 thumb:shadow-contact disabled:checked:track:bg-accent/40 disabled:thumb:shadow-none',
    size: {
      sm: 'track:w-9 track:h-5 thumb:size-4',
      md: 'track:w-11 track:h-[26px] thumb:size-[22px]',
    } satisfies Record<SwitchSize, string>,
    /** Alone, keyboard focus rings the thumb; as a row it is a fill across the row's padding. */
    variant: {
      standalone: 'p-0 focus-visible:thumb:ring-2 focus-visible:thumb:ring-focus',
      row: '-mx-4 px-4 -my-2.5 py-2.5 transition-colors duration-150 ease-out focus-visible:bg-text/7',
    } satisfies Record<SwitchVariant, string>,
  },
  /** Centred against the track; the primitive lays it out beside the track. */
  text: {
    base: 'min-w-0 gap-0.5 self-stretch my-auto',
  },
  label: {
    base: 'text-body text-text',
    disabled: 'text-faint',
  },
  description: {
    base: 'text-footnote text-muted',
  },
} as const;
