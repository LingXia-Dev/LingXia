export const searchFieldParts = {
  root: 'view',
  spinner: 'vector',
  results: 'text',
} as const;

/**
 * A search field is a TextInput with the magnifier and a clear button, so its box is TextInput's
 * recipe. What it adds is the spinner that takes the magnifier's place while a search runs, and
 * the line under it that says what was found.
 */
export const searchFieldRecipe = {
  root: {
    base: 'self-stretch gap-1.5',
  },
  spinner: {
    base: 'size-4 motion-safe:animate-spin',
  },
  results: {
    base: 'text-footnote text-muted tabular-nums',
    hidden: 'sr-only',
  },
} as const;
