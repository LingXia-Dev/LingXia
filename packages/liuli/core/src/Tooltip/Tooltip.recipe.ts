import type { TooltipPlacement } from './Tooltip';

export const tooltipParts = {
  bubble: 'view',
  text: 'text',
} as const;

/** The hint is InfoTip's note: the inverse of the page, small and tight, so it reads as on top. */
export const tooltipRecipe = {
  bubble: {
    base: 'max-w-64 px-3 py-2 rounded-control bg-text shadow-raised transition duration-fast ease-standard starting:opacity-0 exit:opacity-0',
    offset: { bottom: 'mt-1 motion-safe:starting:-translate-y-1', top: 'mb-1 motion-safe:starting:translate-y-1', end: 'ms-1 motion-safe:starting:-translate-x-1 rtl:motion-safe:starting:translate-x-1', start: 'me-1 motion-safe:starting:translate-x-1 rtl:motion-safe:starting:-translate-x-1' } satisfies Record<TooltipPlacement, string>,
  },
  text: {
    base: 'text-footnote text-surface',
  },
} as const;
