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
    base: 'rounded-control disabled:opacity-40',
  },
  row: {
    base: 'flex-row items-start gap-3 py-1',
  },
  box: {
    base: 'items-center justify-center rounded-md border-2 transition-colors duration-150',
    size: { sm: 'size-4 mt-0.5', md: 'size-5 mt-0.5', lg: 'size-6 mt-0.5' } satisfies Record<FieldSize, string>,
    state: {
      on: 'bg-accent border-accent text-on-accent',
      off: 'bg-surface border-line-strong',
    } satisfies Record<'on' | 'off', string>,
    invalid: 'border-danger',
  },
  mark: {
    size: { sm: 'size-3', md: 'size-3.5', lg: 'size-4' } satisfies Record<FieldSize, string>,
  },
  text: {
    base: 'grow basis-0 gap-0.5',
  },
  label: {
    base: 'text-text',
    size: { sm: 'text-sm', md: 'text-base', lg: 'text-lg' } satisfies Record<FieldSize, string>,
    hidden: 'sr-only',
  },
  description: {
    base: 'text-sm text-muted',
  },
} as const;
