import type { DrawerSide, DrawerSize } from './Drawer';

export const drawerParts = {
  panel: 'dialog',
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
 * A panel the height of the screen on one side edge, rounded on its inner side, that slides in from
 * that edge; under reduced motion it fades. The parts match Dialog's.
 */
export const drawerRecipe = {
  panel: {
    base: 'h-full w-[88%] px-5 py-5 gap-4 border-line bg-surface-overlay shadow-modal transition duration-slow ease-standard starting:opacity-0 exit:opacity-0',
    side: {
      end: 'ms-auto me-0 rounded-s-dialog border-s motion-safe:starting:translate-x-full motion-safe:exit:translate-x-full rtl:motion-safe:starting:-translate-x-full rtl:motion-safe:exit:-translate-x-full',
      start: 'me-auto ms-0 rounded-e-dialog border-e motion-safe:starting:-translate-x-full motion-safe:exit:-translate-x-full rtl:motion-safe:starting:translate-x-full rtl:motion-safe:exit:translate-x-full',
    } satisfies Record<DrawerSide, string>,
    size: { sm: 'max-w-80', md: 'max-w-105', lg: 'max-w-140' } satisfies Record<DrawerSize, string>,
  },
  header: {
    base: 'flex-row items-start gap-3',

    /** No title drawn: the close button and any header action keep to the end. */
    bare: 'justify-end',
  },
  title: {
    base: 'grow basis-0 text-title font-semibold tracking-tight text-text',
  },
  /** One small control beside the title, on the close button's line. */
  headerAction: {
    base: 'shrink-0 flex-row items-center gap-1 min-h-8 -mt-1',
  },
  close: {
    base: 'size-8 -me-1.5 -mt-1 items-center justify-center rounded-full text-muted hover:bg-text/5 active:bg-text/8 focus-visible:ring-2 focus-visible:ring-focus focus-visible:ring-offset-2',
  },
  closeIcon: {
    base: 'size-5',
  },
  description: {
    base: 'text-body text-muted whitespace-pre-line',
  },
  body: {
    base: 'grow shrink min-h-0 gap-3 -mx-5 px-5',
  },
  footer: {
    base: 'flex-row flex-wrap justify-end gap-2',
  },
} as const;
