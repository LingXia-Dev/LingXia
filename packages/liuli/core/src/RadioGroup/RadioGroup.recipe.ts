import type { FieldSize } from '../Field/Field';
import type { RadioGroupVariant } from './RadioGroup';

export const radioGroupParts = {
  field: 'Field',
  group: 'view',
  option: 'button',
  line: 'view',
  ghost: 'text',
  circle: 'view',
  dot: 'view',
  text: 'view',
  top: 'view',
  label: 'text',
  end: 'view',
  description: 'text',
} as const;

/** A Field around a radio group: one row per option, a circle drawn from the choice. */
export const radioGroupRecipe = {
  group: {
    base: '',
    variant: { list: 'gap-1', card: 'gap-2' } satisfies Record<RadioGroupVariant, string>,
    /** Side by side: plain rows keep a wide gap so each label stays with its own circle. */
    horizontal: { list: 'flex-row flex-wrap gap-x-6 gap-y-1', card: 'flex-row flex-wrap gap-2' } satisfies Record<RadioGroupVariant, string>,
  },
  option: {
    base: 'flex-row items-start gap-3 text-start',
    variant: {
      /** A row: keyboard focus rings it, with room past the circle and text. */
      list: '-mx-2 px-2 py-1.5 rounded-control transition-colors duration-fast ease-standard hover:bg-text/4 focus-visible:ring-2 focus-visible:ring-focus focus-visible:ring-offset-2',
      card: 'p-4 rounded-container border bg-surface transition-colors duration-fast ease-standard hover:bg-text/3 active:bg-text/6 focus-visible:ring-2 focus-visible:ring-focus focus-visible:ring-offset-2',
    } satisfies Record<RadioGroupVariant, string>,
    /** A card's edge carries the choice: the accent ring, beside the filled circle. */
    card: {
      on: 'border-accent ring-1 ring-accent',
      off: 'border-line',
    } satisfies Record<'on' | 'off', string>,
  },
  /** The label's first line box, holding the mark centred on it at any text size. */
  line: {
    base: 'shrink-0 flex-row items-center',
  },
  /** An empty run in the label's type: it gives the line its height. */
  ghost: {
    base: '',
  },
  circle: {
    // A mark keeps its size and shape beside a label of any length or text size.
    base: 'shrink-0 items-center justify-center rounded-full border-2 transition-colors duration-fast ease-standard',
    // In the label's type, so the mark grows with the text (the View caps that at 1.5×).
    size: { sm: 'text-footnote size-[1.25em]', md: 'text-body size-[1.33em]', lg: 'text-callout size-[1.4em]' } satisfies Record<FieldSize, string>,
    state: {
      on: 'bg-accent-solid border-accent-solid',
      off: 'bg-surface border-line-strong shadow-control',
      /** Disabled reads as off-but-checked: a neutral fill and a muted dot, never a faded accent. */
      onDisabled: 'bg-text/20 border-transparent',
      offDisabled: 'bg-text/4 border-text/12',
    } satisfies Record<'on' | 'off' | 'onDisabled' | 'offDisabled', string>,
    invalid: 'border-danger',
  },
  dot: {
    base: 'size-[0.53em] rounded-full bg-on-accent-solid transition duration-fast ease-standard starting:scale-50 starting:opacity-0',
    disabled: 'bg-muted',
  },
  text: {
    base: 'grow shrink min-w-0 gap-0.5',
  },
  /** The label's line with `endContent` at its end. */
  top: {
    base: 'flex-row items-start gap-3',
  },
  /** At the end of the label's first line; it never gives way, the label wraps instead. */
  end: {
    base: 'shrink-0 flex-row items-center gap-2 text-muted',
    disabled: 'text-faint',
  },
  label: {
    base: 'text-text',
    /** Beside `endContent`: the label takes the line and wraps. */
    beside: 'grow shrink min-w-0',
    disabled: 'text-faint',
    variant: { list: '', card: 'font-medium' } satisfies Record<RadioGroupVariant, string>,
    size: { sm: 'text-footnote', md: 'text-body', lg: 'text-callout' } satisfies Record<FieldSize, string>,
  },
  description: {
    base: 'w-0 min-w-full text-footnote text-muted whitespace-pre-line',
  },
} as const;
