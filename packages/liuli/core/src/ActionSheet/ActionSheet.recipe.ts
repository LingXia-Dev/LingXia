import type { ActionSheetItemVariant } from './ActionSheet';

export const actionSheetParts = {
  panel: 'dialog',
  grabber: 'view',
  header: 'view',
  title: 'text',
  description: 'text',
  actions: 'view',
  item: 'button',
  itemIcon: 'vector',
  itemLabel: 'text',
  cancel: 'view',
} as const;

/**
 * A short list of actions, then Cancel apart from them. On a phone it is a sheet on the bottom edge that slides
 * up and drags off; wider, a panel in the middle that fades in, as narrow as a menu.
 */
export const actionSheetRecipe = {
  panel: {
    base: 'gap-2 border border-line bg-surface-overlay shadow-modal',
    /** On the bottom edge, rounded at the top, past the safe area at the bottom. */
    sheet: 'mt-auto mb-0 w-full max-w-[640px] max-h-[90vh] px-3 pt-2 pb-3 rounded-t-dialog border-b-0 transition duration-slow ease-spring motion-safe:starting:translate-y-full motion-safe:exit:translate-y-full motion-reduce:starting:opacity-0 motion-reduce:exit:opacity-0',
    /** In the middle, as narrow as a menu. */
    floating: 'w-[90%] max-w-[320px] max-h-[85vh] p-2 rounded-dialog transition duration-base ease-standard starting:opacity-0 exit:opacity-0 motion-safe:starting:translate-y-2 motion-safe:exit:translate-y-2',
  },
  grabber: {
    base: 'self-center w-9 h-1 rounded-full bg-text/20',
  },
  /** The title and the message, set in from the rows' text so they line up with it. */
  header: {
    base: 'gap-0.5 px-3 pt-1 pb-1',
  },
  title: {
    base: 'text-footnote font-semibold text-text',
  },
  description: {
    base: 'text-footnote text-muted whitespace-pre-line',
  },
  actions: {
    base: 'self-stretch gap-0.5',
  },
  /** A row: touch height on a phone, a menu's height wider. */
  item: {
    base: 'flex-row items-center gap-3 self-stretch px-3 py-2 rounded-control text-body text-start transition-colors duration-fast ease-standard hover:bg-text/5 active:bg-text/8 focus-visible:bg-text/8 disabled:opacity-40',
    size: { sheet: 'min-h-control-lg', floating: 'min-h-control-md' },
    variant: {
      default: 'text-text',
      destructive: 'text-danger',
    } satisfies Record<ActionSheetItemVariant, string>,
  },
  /** In the row's type, so it grows with the reader's text; a row without one gets an empty box of the same size, so the labels line up. */
  itemIcon: {
    base: 'size-[1.33em] shrink-0',
  },
  itemLabel: {
    base: 'grow shrink min-w-0 text-body',
  },
  /** Cancel stands apart, under a hairline. */
  cancel: {
    base: 'self-stretch pt-2 border-t border-line',
  },
} as const;
