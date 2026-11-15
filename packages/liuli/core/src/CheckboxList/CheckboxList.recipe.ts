import type { FieldSize } from '../Field/Field';

export const checkboxListParts = {
  root: 'view',
  label: 'text',
  description: 'text',
  options: 'view',
  all: 'CheckboxInput',
} as const;

/**
 * A question and its checkboxes. The question is set as a field label; the options are
 * CheckboxInput rows, closer to each other than to the question.
 */
export const checkboxListRecipe = {
  root: {
    base: 'gap-2',
  },
  label: {
    base: 'font-medium text-text',
    size: { sm: 'text-caption', md: 'text-footnote', lg: 'text-body' } satisfies Record<FieldSize, string>,
    disabled: 'text-faint',
  },
  description: {
    base: 'text-footnote text-muted',
  },
  /** The box for all sits over a hairline, apart from the options it stands for. */
  all: {
    base: 'pb-2.5 border-b border-text/8',
  },
  options: {
    size: { sm: 'gap-2', md: 'gap-2.5', lg: 'gap-3' } satisfies Record<FieldSize, string>,
  },
} as const;
