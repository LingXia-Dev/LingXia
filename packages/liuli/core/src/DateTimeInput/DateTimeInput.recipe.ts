export const dateTimeInputParts = {
  group: 'view',
  date: 'DateInput',
  time: 'TimeInput',
} as const;

/**
 * The date and the time side by side under one label while both fit at their base widths; in a
 * narrow place the time goes under the date and both take the whole width. A part's least width is
 * its value's: the View keeps a date or time editor as wide as its widest value.
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
} as const;
