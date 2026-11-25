import type { ToastPosition, ToastTone } from './Toast';

export const toastParts = {
  region: 'view',
  toast: 'view',
  icon: 'vector',
  message: 'text',
  action: 'Button',
  dismiss: 'button',
  announcer: 'text',
} as const;

/**
 * A raised card on the page's surface, in a corner of the screen, the bottom end unless the region says
 * otherwise. It moves in a little from its edge as it fades in; under reduced motion it only fades.
 */
export const toastRecipe = {
  /** The top-layer box the toast sits in. It meets every edge, so the layer adds the bars to its 16 px gap. */
  region: {
    base: 'absolute inset-0 m-0 p-4 transition duration-base ease-standard starting:opacity-0 exit:opacity-0',
    position: {
      bottomEnd: 'justify-end items-end motion-safe:starting:translate-y-2 motion-safe:exit:translate-y-2',
      bottomStart: 'justify-end items-start motion-safe:starting:translate-y-2 motion-safe:exit:translate-y-2',
      topEnd: 'justify-start items-end motion-safe:starting:-translate-y-2 motion-safe:exit:-translate-y-2',
      topStart: 'justify-start items-start motion-safe:starting:-translate-y-2 motion-safe:exit:-translate-y-2',
    } satisfies Record<ToastPosition, string>,
  },
  toast: {
    base: 'flex-row items-center gap-3 min-h-12 ps-4 pe-1.5 py-1.5 rounded-container border border-text/12 bg-surface-overlay shadow-raised transition-transform duration-slow ease-standard',
    /** As wide as the region up to 400 pt, as the reference's: the screen's width on a phone, a corner card on a wide one. */
    width: 'w-full max-w-[400px]',
    /** Without a dismiss button the end padding matches the start. */
    plain: 'pe-4',
  },
  icon: {
    base: 'size-5 shrink-0',
    tone: { neutral: 'text-muted', info: 'text-info', success: 'text-success', warning: 'text-warning', danger: 'text-danger' } satisfies Record<ToastTone, string>,
  },
  message: {
    base: 'grow shrink basis-0 min-w-0 py-1.5 text-body text-text whitespace-pre-line',
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
  /** Mounted for the region's whole life, so a new message is a change it announces. */
  announcer: {
    base: 'sr-only',
  },
} as const;
