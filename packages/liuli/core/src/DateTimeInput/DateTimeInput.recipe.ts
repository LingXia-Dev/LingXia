export const dateTimeInputParts = {
  group: 'view',
  date: 'DateInput',
  time: 'TimeInput',
} as const;

/**
 * The date and the time side by side under one label while both fit at their least widths; in a
 * narrow place the time goes under the date and both take the whole width.
 */
export const dateTimeInputRecipe = {
  group: {
    base: 'flex-row flex-wrap items-start gap-2',
  },
  date: {
    base: 'grow-[2] basis-40 min-w-40',
  },
  time: {
    base: 'grow basis-28 min-w-28',
  },
} as const;
