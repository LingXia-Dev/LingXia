import type { FieldSize } from '../Field/Field';

export const checkboxInputParts = {
  root: 'view',
  control: 'checkbox',
  row: 'view',
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
    base: '-mx-2 px-2 -my-1 py-1 rounded-control transition-colors duration-fast ease-standard disabled:opacity-40 hover:bg-text/4 focus-visible:bg-text/7',
  },
  /** Centred in the primitive's minimum height when the content is shorter. */
  row: {
    base: 'flex-row items-start gap-3 py-1 my-auto',
  },
  box: {
    // A mark keeps its size and shape beside a label of any length or text size.
    base: 'shrink-0 items-center justify-center rounded-checkbox border-2 transition-colors duration-fast ease-standard',
    /** Centred on the label's first line. */
    size: { sm: 'size-4 mt-px', md: 'size-5 mt-px', lg: 'size-6' } satisfies Record<FieldSize, string>,
    state: {
      on: 'bg-accent-solid border-accent-solid text-on-accent-solid',
      off: 'bg-surface border-line-strong shadow-[0_1px_2px_rgb(0_0_0/0.05)]',
    } satisfies Record<'on' | 'off', string>,
    invalid: 'border-danger',
  },
  mark: {
    size: { sm: 'size-3', md: 'size-3.5', lg: 'size-4' } satisfies Record<FieldSize, string>,
  },
  /** Partly chosen: a dash on the filled box. */
  dash: {
    base: 'h-0.5 rounded-full bg-on-accent',
    size: { sm: 'w-2', md: 'w-2.5', lg: 'w-3' } satisfies Record<FieldSize, string>,
  },
  text: {
    base: 'grow shrink min-w-0 gap-0.5',
  },
  label: {
    base: 'text-text',
    size: { sm: 'text-footnote', md: 'text-body', lg: 'text-callout' } satisfies Record<FieldSize, string>,
    hidden: 'sr-only',
  },
  description: {
    base: 'text-footnote text-muted',
  },
  /** The status message starts under the label: the box width plus the row gap. */
  status: {
    size: { sm: 'ps-7', md: 'ps-8', lg: 'ps-9' } satisfies Record<FieldSize, string>,
  },
} as const;
