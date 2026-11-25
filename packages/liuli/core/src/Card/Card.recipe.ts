import type { CardPadding, CardVariant } from './Card';

export const cardParts = { root: 'view', description: 'text', group: 'view' } as const;

/** Card classes. ClickableCard lands the same root classes on a `button`, plus `pressable`. */
export const cardRecipe = {
  root: {
    base: 'rounded-container text-start',
    variant: {
      outlined: 'bg-surface border border-line',
      raised: 'bg-surface-raised border border-line shadow-contact',
      floating: 'bg-surface-raised border border-line shadow-raised',
      // Dark `surface-sunken` sits a hair under the canvas; the hairline keeps the well's edge on any surface and in high contrast.
      sunken: 'bg-surface-sunken border border-line',
    } satisfies Record<CardVariant, string>,
    padding: {
      none: 'p-0 gap-0',
      sm: 'p-3 gap-2',
      md: 'p-4 gap-3',
      lg: 'p-6 gap-4',
    } satisfies Record<CardPadding, string>,
    // A press is a wash over the card's own surface: an inset ring wide enough to fill it, so the fill underneath stays.
    pressable: 'items-stretch transition-colors duration-fast ease-standard hover:border-line-strong active:inset-ring-[999px] active:inset-ring-text/7 disabled:opacity-40 focus-visible:ring-2 focus-visible:ring-focus focus-visible:ring-offset-2',
    /** A wider accent border; it swaps colour and width, never the card's size. */
    selected: 'border-accent ring-1 ring-accent',
  },
  /** ClickableCardGroup's radio group; the caller's className lays the cards out. */
  /** ClickableCard's `description`: read after the name, never drawn; the card's content shows it. */
  description: {
    base: 'sr-only',
  },
  group: {
    base: 'gap-3',
  },
} as const;
