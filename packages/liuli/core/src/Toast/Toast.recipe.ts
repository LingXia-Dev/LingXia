import type { ToastPosition, ToastTone } from './Toast';

export const toastParts = {
  region: 'view',
  toast: 'view',
  icon: 'vector',
  spinner: 'view',
  stack: 'view',
  message: 'text',
  action: 'Button',
  dismiss: 'button',
  announcer: 'text',
  content: 'view',
} as const;

/**
 * A raised card on the page's surface, in a corner of the screen, the bottom end unless the region says
 * otherwise. It moves in a little from its edge as it fades in; under reduced motion it only fades.
 */
export const toastRecipe = {
  /**
   * The top-layer box the toast sits in: only the toast and its 16 px gap, never the page, since a
   * press on a top-layer box never reaches what is under it. It meets its corner's edges, so the layer adds the bars there.
   */
  region: {
    base: 'fixed m-0 p-4 max-w-full transition duration-base ease-standard starting:opacity-0 exit:opacity-0',
    position: {
      bottomEnd: 'bottom-0 end-0 justify-end items-end motion-safe:starting:translate-y-2 motion-safe:exit:translate-y-2',
      bottomStart: 'bottom-0 start-0 justify-end items-start motion-safe:starting:translate-y-2 motion-safe:exit:translate-y-2',
      topEnd: 'top-0 end-0 justify-start items-end motion-safe:starting:-translate-y-2 motion-safe:exit:-translate-y-2',
      topStart: 'top-0 start-0 justify-start items-start motion-safe:starting:-translate-y-2 motion-safe:exit:-translate-y-2',
    } satisfies Record<ToastPosition, string>,
    /**
     * A definite width, so the card's message wraps inside it: on a touch screen the whole row, the card
     * taking it up to its 400 pt; with a pointer the 400 pt card and its gap.
     */
    width: { touch: 'inset-x-0 w-full', pointer: 'w-[432px]' },
  },
  toast: {
    base: 'flex-row items-center gap-3 min-h-12 ps-4 pe-1.5 py-1.5 rounded-container border border-line bg-surface-overlay shadow-raised transition-transform duration-slow ease-standard',
    /** As wide as the region up to 400 pt, as the reference's: the screen's width on a phone, a corner card on a wide one. */
    width: 'w-full max-w-[400px]',
    /** Without a dismiss button the end padding matches the start. */
    plain: 'pe-4',
  },
  /** 20 pt, grown with the reader's text; the glyph and the busy spinner share it. */
  icon: {
    base: 'size-5 shrink-0',
    tone: { neutral: 'text-muted', info: 'text-info', success: 'text-success', warning: 'text-warning', danger: 'text-danger' } satisfies Record<ToastTone, string>,
  },
  /** Busy: turns in the glyph's place; still under reduced motion. */
  spinner: {
    base: 'motion-safe:animate-spin',
  },
  /** At accessibility text sizes: the message over its action, taking the card's width. */
  stack: {
    base: 'grow shrink basis-0 min-w-0 items-start pb-1',
  },
  message: {
    /** The message's line: the icon's slot centres on one of it. */
    type: 'py-1.5 text-body',
    base: 'grow shrink basis-0 min-w-0 py-1.5 text-body text-text whitespace-pre-line',
    under: 'self-stretch py-1.5 text-body text-text whitespace-pre-line',
  },
  action: {
    base: 'shrink-0',
  },
  dismiss: {
    base: 'size-9 shrink-0 items-center justify-center rounded-full text-muted hover:bg-text/6 active:bg-text/10 focus-visible:ring-2 focus-visible:ring-focus focus-visible:ring-offset-2',
  },
  dismissIcon: {
    base: 'size-4',
  },
  /** The page's content, in one box a context can wrap; it fills the page, and a page-bottom scroll inside it still runs under the bars. */
  content: {
    base: 'flex-1 min-h-0',
  },
  /** Mounted for the region's whole life, so a new message is a change it announces. */
  announcer: {
    base: 'sr-only',
  },
} as const;
