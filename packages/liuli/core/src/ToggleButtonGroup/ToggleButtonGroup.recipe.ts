export const toggleButtonGroupParts = { root: 'view' } as const;

/** A row of toggles with a gap, wrapping when it does not fit; each toggle is drawn by ToggleButton's recipe. */
export const toggleButtonGroupRecipe = {
  root: {
    base: 'flex-row flex-wrap items-center self-start max-w-full gap-1',
  },
} as const;
