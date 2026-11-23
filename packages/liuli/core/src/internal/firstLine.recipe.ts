/** The slot that centres leading content on the first line of the text beside it. */
export const firstLineRecipe = {
  root: {
    base: 'shrink-0 flex-row items-center',
    along: { first: 'self-start', centre: 'self-center' },
  },
  /** One line of the text's style, drawn nowhere. */
  strut: 'w-0',
} as const;
