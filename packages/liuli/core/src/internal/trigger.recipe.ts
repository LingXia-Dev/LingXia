/** The plain button around a trigger that is not a Liuli button: no look of its own, a focus ring around the child. */
export const triggerRecipe = {
  wrap: {
    base: 'self-start rounded-control focus-visible:ring-2 focus-visible:ring-focus focus-visible:ring-offset-2',
    /** Words that show a hint say so with a dotted underline; a Liuli pressable or a Link draws its own look. */
    hint: 'underline decoration-dotted decoration-line-strong',
  },
} as const;
