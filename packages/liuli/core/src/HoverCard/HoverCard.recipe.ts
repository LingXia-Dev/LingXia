import type { HoverCardPlacement } from './HoverCard';

export const hoverCardParts = {
  trigger: 'button',
  card: 'view',
} as const;

/** The card is Popover\'s panel, a little narrower; a wrapped trigger keeps its words' own look. */
export const hoverCardRecipe = {
  trigger: {
    // The ring around a few words takes the smallest corner role.
    base: 'self-start rounded-checkbox focus-visible:ring-2 focus-visible:ring-focus focus-visible:ring-offset-2',
  },
  card: {
    base: 'w-64 p-4 gap-2 rounded-container border border-line bg-surface-overlay shadow-raised transition duration-fast ease-standard starting:opacity-0 exit:opacity-0',
    offset: { bottom: 'mt-1.5', top: 'mb-1.5', end: 'ms-1.5', start: 'me-1.5' } satisfies Record<HoverCardPlacement, string>,
  },
} as const;
