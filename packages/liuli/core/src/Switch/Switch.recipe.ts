import type { SwitchSize, SwitchVariant } from './Switch';

export const switchParts = {
  root: 'view',
  control: 'switch',
  text: 'view',
  loading: 'view',
  spinner: 'vector',
  label: 'text',
  description: 'text',
  failed: 'view',
} as const;

/**
 * The switch primitive holds the label and help text as its content and draws the track and thumb
 * at its end, so the whole block is the press target. The thumb is one white knob in every state;
 * on, the track is accent-solid, the fill a primary Button has.
 */
export const switchRecipe = {
  root: {
    base: '',
    variant: { standalone: '', row: 'self-stretch px-4 py-2.5 min-h-control-lg justify-center' } satisfies Record<SwitchVariant, string>,
  },
  control: {
    base: 'min-h-8 track:rounded-full track:bg-line-strong/45 checked:track:bg-accent-solid thumb:rounded-full thumb:bg-knob thumb:border thumb:border-line thumb:shadow-contact disabled:track:bg-text/12 disabled:checked:track:bg-text/20',
    /** With the label hidden: the track alone, as wide as it is, with a spinner before it while loading. */
    bare: 'self-start',
    /** Busy, not unavailable: the track keeps its colour while presses are held off. */
    loading: 'disabled:track:bg-line-strong/45 disabled:checked:track:bg-accent-solid',
    size: {
      sm: 'track:w-9 track:h-5 thumb:size-4',
      md: 'track:w-11 track:h-[26px] thumb:size-[22px]',
    } satisfies Record<SwitchSize, string>,
    /** Alone, keyboard focus rings the thumb; as a row it rings the row inside its edges, which run to the list's. */
    variant: {
      standalone: 'p-0 focus-visible:thumb:ring-2 focus-visible:thumb:ring-focus focus-visible:thumb:ring-offset-2',
      row: '-mx-4 px-4 -my-2.5 py-2.5 transition-colors duration-fast ease-standard hover:bg-text/4 focus-visible:inset-ring-2 focus-visible:inset-ring-focus',
    } satisfies Record<SwitchVariant, string>,
  },
  /** Centred against the track; the primitive lays it out beside the track. */
  text: {
    base: 'min-w-0 gap-0.5 self-stretch my-auto',
  },
  /** While loading: the words, then the spinner beside the track. */
  loading: {
    base: 'flex-row items-center gap-3 self-stretch my-auto',
    words: 'grow basis-0 min-w-0',
  },
  spinner: {
    base: 'size-4 shrink-0 text-muted motion-safe:animate-spin',
  },
  label: {
    base: 'text-body text-text',
    disabled: 'text-faint',
  },
  description: {
    base: 'text-footnote text-muted whitespace-pre-line',
  },
  /** A failed `changeAction`, under the label, until the next change. */
  failed: {
    base: 'text-footnote',
  },
} as const;
