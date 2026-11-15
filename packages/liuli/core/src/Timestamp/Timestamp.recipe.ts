export const timestampParts = { root: 'text' } as const;

/** The size and colour come from where it sits; only the digits are fixed. */
export const timestampRecipe = {
  root: {
    base: 'tabular-nums',
  },
} as const;
