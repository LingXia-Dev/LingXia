export const iconButtonParts = { root: 'button' } as const;

/**
 * An icon button is a Button drawn as a square with its icon alone, so its look is Button's
 * recipe. What it adds is here: it never shrinks or stretches beside its neighbours.
 */
export const iconButtonRecipe = {
  root: {
    base: 'shrink-0 self-center',
  },
} as const;
