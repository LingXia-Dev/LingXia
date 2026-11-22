import type { TourPlacement } from './Tour';

export const tourParts = {
  callout: 'view',
  caret: 'view',
  title: 'text',
  text: 'text',
  footer: 'view',
  count: 'text',
  actions: 'view',
  quiet: 'Button',
} as const;

/** A callout on the inverted surface, as InfoTip\'s note is, so it reads as apart from the page it points at. */
export const tourRecipe = {
  /** Anchored to the target and kept clear of it by its margin, on whichever side it lands. */
  callout: {
    base: 'w-72 m-3 p-4 gap-2 rounded-container bg-text shadow-raised transition duration-fast ease-standard starting:opacity-0 exit:opacity-0',
  },
  /**
   * A square turned on its corner, half out of the callout's edge facing the target, on the target's
   * centre line even when the callout is pushed in from a screen edge. Hidden until the callout lands on its side.
   */
  caret: {
    base: 'absolute size-3 rotate-45 rounded-[2px] bg-text opacity-0',
    side: {
      bottom: '-top-1.5 start-[anchor(center)] -ms-1.5 side-bottom:opacity-100',
      top: '-bottom-1.5 start-[anchor(center)] -ms-1.5 side-top:opacity-100',
      end: '-start-1.5 top-[anchor(center)] -mt-1.5 side-end:opacity-100',
      start: '-end-1.5 top-[anchor(center)] -mt-1.5 side-start:opacity-100',
    } satisfies Record<TourPlacement, string>,
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
