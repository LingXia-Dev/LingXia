import type { HoverCardPlacement } from './HoverCard';

export const hoverCardParts = {
  trigger: 'button',
  label: 'text',
  card: 'view',
} as const;

/** The trigger reads as a term with a dotted underline, apart from a link's solid one; the card is Popover\'s panel, a little narrower. */
export const hoverCardRecipe = {
  trigger: {
    base: 'self-start rounded-[4px] focus-visible:ring-2 focus-visible:ring-focus focus-visible:ring-offset-2',
  },
  label: {
    base: 'text-text underline decoration-dotted decoration-line-strong',
  },
  card: {
    base: 'w-64 p-4 gap-2 rounded-container border border-text/12 bg-surface-overlay shadow-raised transition duration-fast ease-standard starting:opacity-0 exit:opacity-0',
    offset: { bottom: 'mt-1.5', top: 'mb-1.5', end: 'ms-1.5', start: 'me-1.5' } satisfies Record<HoverCardPlacement, string>,
  },
} as const;
