import type { FieldSize } from '../Field/Field';

export const dateTimeInputParts = {
  group: 'view',
  date: 'DateInput',
  time: 'TimeInput',
  measure: 'view',
  sample: 'text',
} as const;

/**
 * The date and the time side by side under one label while both fit; in a narrow place, or at a
 * text size where their values no longer fit beside each other, the time goes under the date and
 * both take the whole width.
 */
export const dateTimeInputRecipe = {
  group: {
    base: 'flex-row flex-wrap items-start gap-2',
  },
  date: {
    base: 'grow-[2] basis-40',
  },
  time: {
    base: 'grow basis-28',
  },
  /**
   * `hasClear`: the time part holds the clear button, so it starts wider and the date narrower; the two
   * still start at the plain pair's width, so the pair wraps where a plain one does unless the values need more.
   */
  clearable: {
    date: 'grow-[2] basis-32',
    time: 'grow basis-36',
  },
  /** Measured as wide as their values: each part on its own line. */
  stacked: 'basis-full',
  /** A long date and time in the editor's type, measured and never seen. */
  measure: {
    base: 'absolute size-0 overflow-hidden flex-row',
    sample: 'shrink-0 whitespace-nowrap tabular-nums',
    size: { sm: 'text-footnote', md: 'text-body', lg: 'text-callout' } satisfies Record<FieldSize, string>,
  },
} as const;
