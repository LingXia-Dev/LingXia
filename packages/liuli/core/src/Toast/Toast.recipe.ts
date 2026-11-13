import type { ToastTone } from './Toast';

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
 * A raised card on the page's surface, at the bottom of the screen on touch and the bottom end with
 * a pointer. It rises a little as it fades in; under reduced motion it only fades.
 */
export const toastRecipe = {
  /** The top-layer box the toast sits in; its padding, the 16 px gap plus the bars, is set from the environment. */
  region: {
    base: 'absolute inset-0 m-0 justify-end transition duration-200 ease-out starting:opacity-0 exit:opacity-0 motion-safe:starting:translate-y-2 motion-safe:exit:translate-y-2',
    pointer: { coarse: 'items-center', fine: 'items-end' },
  },
  toast: {
    base: 'flex-row items-center gap-3 min-h-12 ps-4 pe-1.5 py-1.5 rounded-container border border-text/12 bg-surface-raised shadow-raised transition-transform duration-300 ease-out',
    pointer: { coarse: 'w-full max-w-[480px]', fine: 'w-[360px] max-w-full' },
    /** Without a dismiss button the end padding matches the start. */
    plain: 'pe-4',
  },
  icon: {
    base: 'size-5 shrink-0',
    tone: { neutral: 'text-muted', info: 'text-accent', success: 'text-success', warning: 'text-warning', danger: 'text-danger' } satisfies Record<ToastTone, string>,
  },
  message: {
    base: 'grow shrink basis-0 min-w-0 py-1.5 text-body text-text',
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
