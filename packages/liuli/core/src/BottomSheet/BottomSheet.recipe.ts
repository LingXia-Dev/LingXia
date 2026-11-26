import type { BottomSheetDetents } from './BottomSheet';

export const bottomSheetParts = {
  panel: 'dialog',
  grabber: 'view',
  header: 'view',
  title: 'text',
  headerAction: 'view',
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
    base: 'mt-auto mb-0 w-full max-w-160 px-5 pt-2 pb-5 gap-4 rounded-t-dialog border border-b-0 border-line bg-surface-overlay shadow-modal transition duration-slow ease-spring motion-safe:starting:translate-y-full motion-safe:exit:translate-y-full motion-reduce:starting:opacity-0 motion-reduce:exit:opacity-0',
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

    /** No title drawn: the close button and any header action keep to the end. */
    bare: 'justify-end',
  },
  title: {
    base: 'grow basis-0 pt-1 text-title font-semibold tracking-tight text-text',
  },
  /** One small control beside the title, on the close button's line. */
  headerAction: {
    base: 'shrink-0 flex-row items-center gap-1 min-h-8',
  },
  close: {
    base: 'size-8 -me-1.5 items-center justify-center rounded-full text-muted hover:bg-text/5 active:bg-text/8 focus-visible:ring-2 focus-visible:ring-focus focus-visible:ring-offset-2',
  },
  closeIcon: {
    base: 'size-5',
  },
  description: {
    base: '-mt-2 text-body text-muted whitespace-pre-line',
  },
  body: {
    base: 'shrink grow min-h-0 gap-3 -mx-5 px-5 overscroll-contain',
  },
  footer: {
    base: 'flex-row flex-wrap justify-end gap-2',
  },
} as const;
