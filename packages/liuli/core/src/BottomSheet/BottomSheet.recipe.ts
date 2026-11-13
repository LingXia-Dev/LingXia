import type { BottomSheetDetents } from './BottomSheet';

export const bottomSheetParts = {
  panel: 'dialog',
  grabber: 'view',
  header: 'view',
  title: 'text',
  close: 'button',
  closeIcon: 'vector',
  description: 'text',
  body: 'scroll',
  footer: 'view',
} as const;

/**
 * A panel on the bottom edge, rounded at the top, that slides up to open and follows the finger
 * between its detents; under reduced motion it fades. On a wide screen it keeps to 640 px, centred.
 */
export const bottomSheetRecipe = {
  panel: {
    base: 'mt-auto mb-0 w-full max-w-[640px] px-5 pt-2 gap-4 rounded-t-dialog border border-b-0 border-text/10 bg-surface-raised shadow-modal transition duration-300 ease-spring motion-safe:starting:translate-y-full motion-safe:exit:translate-y-full motion-reduce:starting:opacity-0 motion-reduce:exit:opacity-0',
    detents: {
      fit: 'max-h-[90vh]',
      expandable: 'h-[90vh]',
    } satisfies Record<BottomSheetDetents, string>,
  },
  /** The handle that says the sheet moves; decoration, as the sheet itself is the adjustable. */
  grabber: {
    base: 'self-center w-9 h-1 rounded-full bg-text/20',
  },
  header: {
    base: 'flex-row items-start gap-3 min-h-8',
  },
  title: {
    base: 'grow basis-0 pt-1 text-title font-semibold tracking-tight text-text',
  },
  close: {
    base: 'size-8 -me-1.5 items-center justify-center rounded-full text-muted hover:bg-text/5 active:bg-text/8 focus-visible:ring-2 focus-visible:ring-focus focus-visible:ring-offset-2',
  },
  closeIcon: {
    base: 'size-5',
  },
  description: {
    base: '-mt-2 text-body text-muted',
  },
  body: {
    base: 'shrink grow min-h-0 gap-3 overscroll-contain',
  },
  footer: {
    base: 'flex-row flex-wrap justify-end gap-2',
  },
} as const;
