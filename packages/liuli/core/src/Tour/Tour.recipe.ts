import type { TourPlacement } from './Tour';

export const tourParts = {
  caret: 'view',
  point: 'view',
  callout: 'view',
  title: 'text',
  text: 'text',
  footer: 'view',
  count: 'text',
  actions: 'view',
  quiet: 'Button',
} as const;

/** A callout on the inverted surface, as Tooltip\'s bubble is, so it reads as apart from the page it points at. */
export const tourRecipe = {
  /**
   * A square turned on its corner, anchored to the target: it stays on the target's centre. The
   * callout hangs from the caret, is shifted to fit the screen, and covers the caret's far half.
   */
  caret: {
    base: 'size-3 overflow-visible bg-transparent transition duration-fast ease-standard starting:opacity-0 exit:opacity-0',
    point: 'size-3 rotate-45 rounded-[2px] bg-text',
    /** Moved across the gap the View keeps between an anchor and its popover, so its far half tucks under the callout. */
    offset: { bottom: 'translate-y-2', top: '-translate-y-2', end: 'translate-x-2 rtl:-translate-x-2', start: '-translate-x-2 rtl:translate-x-2' } satisfies Record<TourPlacement, string>,
  },
  callout: {
    base: 'w-72 p-4 gap-2 rounded-container bg-text shadow-raised transition duration-fast ease-standard starting:opacity-0 exit:opacity-0',
    offset: { bottom: '', top: '', end: '', start: '' } satisfies Record<TourPlacement, string>,
  },
  title: {
    base: 'text-body font-semibold text-surface',
  },
  text: {
    base: 'text-footnote text-surface',
  },
  footer: {
    base: 'flex-row items-center justify-between gap-3 pt-1',
  },
  count: {
    base: 'text-caption text-surface/70 tabular-nums',
  },
  actions: {
    base: 'flex-row items-center gap-1',
  },
  /** Ghost buttons on the inverted surface take its text colour. */
  quiet: {
    base: 'text-surface hover:bg-surface/10 active:bg-surface/15',
  },
} as const;
