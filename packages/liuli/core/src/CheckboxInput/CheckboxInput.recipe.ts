import type { FieldSize } from '../Field/Field';

export const checkboxInputParts = {
  root: 'view',
  control: 'checkbox',
  row: 'view',
  box: 'view',
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
    base: 'p-0 rounded-control disabled:opacity-40',
  },
  row: {
    base: 'flex-row items-start gap-3 py-1',
  },
  box: {
    base: 'items-center justify-center rounded-checkbox border-2 transition-colors duration-150 ease-out',
    /** Centred on the label's first line. */
    size: { sm: 'size-4 mt-px', md: 'size-5 mt-px', lg: 'size-6' } satisfies Record<FieldSize, string>,
    state: {
      on: 'bg-accent border-accent text-on-accent',
      off: 'bg-surface border-line-strong shadow-[0_1px_2px_rgb(0_0_0/0.05)]',
    } satisfies Record<'on' | 'off', string>,
    invalid: 'border-danger',
  },
  mark: {
    size: { sm: 'size-3', md: 'size-3.5', lg: 'size-4' } satisfies Record<FieldSize, string>,
  },
  text: {
    base: 'grow shrink min-w-0 gap-0.5',
  },
  label: {
    base: 'text-text',
    size: { sm: 'text-footnote', md: 'text-body', lg: 'text-base' } satisfies Record<FieldSize, string>,
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
