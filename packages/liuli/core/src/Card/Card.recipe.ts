export const cardParts = { root: 'view' } as const;

/** Card classes. ClickableCard lands the same root classes on a `button`, plus `pressable`. */
export const cardRecipe = {
  root: {
    base: 'rounded-container text-start',
    variant: {
      outlined: 'bg-surface border border-line',
      raised: 'bg-surface-raised border border-line shadow-md',
      sunken: 'bg-surface-sunken border border-transparent',
    },
    padding: {
      none: 'p-0 gap-0',
      sm: 'p-3 gap-2',
      md: 'p-4 gap-3',
      lg: 'p-6 gap-4',
    },
    pressable: 'items-stretch transition-colors duration-150 hover:border-line-strong active:bg-surface-sunken disabled:opacity-40',
    /** A wider accent border; it swaps colour and width, never the card's size. */
    selected: 'border-accent ring-1 ring-accent',
  },
} as const;
