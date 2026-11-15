import type { TooltipPlacement, TooltipSize } from './Tooltip';

export const tooltipParts = {
  trigger: 'button',
  icon: 'vector',
  bubble: 'view',
  text: 'text',
} as const;

/**
 * An info button and the short note it opens. The note is the inverse of the page, small and
 * tight, so it reads as a note on top rather than another card.
 */
export const tooltipRecipe = {
  trigger: {
    base: 'items-center justify-center rounded-full text-muted transition-colors duration-150 ease-out hover:bg-text/6 active:bg-text/10 focus-visible:ring-2 focus-visible:ring-focus focus-visible:ring-offset-2',
    /** The glyph stays small; the press area does not. */
    size: { sm: 'size-8', md: 'size-10' } satisfies Record<TooltipSize, string>,
  },
  icon: {
    size: { sm: 'size-4', md: 'size-5' } satisfies Record<TooltipSize, string>,
  },
  bubble: {
    base: 'max-w-64 px-3 py-2 rounded-control bg-text shadow-raised transition duration-150 ease-out starting:opacity-0 exit:opacity-0 motion-safe:starting:scale-95',
    offset: { bottom: 'mt-1 origin-top', top: 'mb-1 origin-bottom', end: 'ms-1', start: 'me-1' } satisfies Record<TooltipPlacement, string>,
  },
  text: {
    base: 'text-footnote text-surface',
  },
} as const;
