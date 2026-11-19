import type { FieldSize } from '../Field/Field';

export const checkboxInputParts = {
  root: 'view',
  control: 'checkbox',
  row: 'view',
  line: 'view',
  ghost: 'text',
  box: 'view',
  dash: 'view',
  mark: 'vector',
  text: 'view',
  label: 'text',
  description: 'text',
  status: 'view',
} as const;

/**
 * A composed `<checkbox>`: with content of its own the primitive draws no box, so Liuli draws it
 * from the value. The checkbox has no flex direction of its own, so an inner row lays it out.
 */
export const checkboxInputRecipe = {
  root: {
    base: 'gap-1',
  },
  control: {
    /** A row: keyboard focus is a fill, given room past the box and text by the negative margins. */
    base: '-mx-2 px-2 -my-1 py-1 rounded-control transition-colors duration-fast ease-standard hover:bg-text/4 focus-visible:bg-text/7',
  },
  /** Centred in the primitive's minimum height when the content is shorter. */
  row: {
    base: 'flex-row items-start gap-3 py-1 my-auto',
  },
  /** The label's first line box, holding the mark centred on it at any text size. */
  line: {
    base: 'shrink-0 flex-row items-center',
  },
  /** An empty run in the label's type: it gives the line its height. */
  ghost: {
    base: 'opacity-0',
  },
  box: {
    // A mark keeps its size and shape beside a label of any length or text size.
    base: 'shrink-0 items-center justify-center rounded-checkbox border-2 transition-colors duration-fast ease-standard',
    // In the label's type, so the mark grows with the text (the View caps that at 1.5×).
    size: { sm: 'text-footnote size-[1.25em]', md: 'text-body size-[1.33em]', lg: 'text-callout size-[1.4em]' } satisfies Record<FieldSize, string>,
    state: {
      on: 'bg-accent-solid border-accent-solid text-on-accent-solid',
      off: 'bg-surface border-line-strong shadow-[0_1px_2px_rgb(0_0_0/0.05)]',
      /** Disabled reads as off-but-checked: a neutral fill and a muted mark, never a faded accent. */
      onDisabled: 'bg-text/20 border-transparent text-muted',
      offDisabled: 'bg-text/4 border-text/12',
    } satisfies Record<'on' | 'off' | 'onDisabled' | 'offDisabled', string>,
    invalid: 'border-danger',
  },
  mark: {
    base: 'size-[0.93em]',
  },
  /** Partly chosen: a dash on the filled box. */
  dash: {
    base: 'h-0.5 w-[0.66em] rounded-full bg-on-accent-solid',
    disabled: 'bg-muted',
  },
  text: {
    base: 'grow shrink min-w-0 gap-0.5',
  },
  label: {
    base: 'text-text',
    disabled: 'text-faint',
    size: { sm: 'text-footnote', md: 'text-body', lg: 'text-callout' } satisfies Record<FieldSize, string>,
    hidden: 'sr-only',
  },
  description: {
    base: 'text-footnote text-muted',
  },
  /** The status message starts under the label: a spacer as wide as the box, then the row gap. */
  status: {
    base: 'flex-row gap-3',
    // Width takes `em` today; padding does not yet, so the indent is a spacer.
    spacer: { sm: 'shrink-0 text-footnote w-[1.25em]', md: 'shrink-0 text-body w-[1.33em]', lg: 'shrink-0 text-callout w-[1.4em]' } satisfies Record<FieldSize, string>,
    region: 'grow shrink min-w-0',
  },
} as const;
