import type { CardPadding, CardVariant } from './Card';

export const cardParts = { root: 'view', group: 'view' } as const;

/** Card classes. ClickableCard lands the same root classes on a `button`, plus `pressable`. */
export const cardRecipe = {
  root: {
    base: 'rounded-container text-start',
    variant: {
      outlined: 'bg-surface border border-text/12',
      raised: 'bg-surface-raised border border-text/12 shadow-contact',
      floating: 'bg-surface-raised border border-text/8 shadow-raised',
      sunken: 'bg-surface-sunken border border-transparent',
    } satisfies Record<CardVariant, string>,
    padding: {
      none: 'p-0 gap-0',
      sm: 'p-3 gap-2',
      md: 'p-4 gap-3',
      lg: 'p-6 gap-4',
    } satisfies Record<CardPadding, string>,
    // A press is a wash over the card's own surface: an inset ring wide enough to fill it, so the fill underneath stays.
    pressable: 'items-stretch transition-colors duration-150 ease-out hover:border-text/20 active:inset-ring-[999px] active:inset-ring-text/7 disabled:opacity-40 focus-visible:ring-2 focus-visible:ring-focus focus-visible:ring-offset-2',
    /** A wider accent border; it swaps colour and width, never the card's size. */
    selected: 'border-accent ring-1 ring-accent',
  },
  /** ClickableCardGroup's radio group; the caller's className lays the cards out. */
  group: {
    base: 'gap-3',
  },
} as const;
