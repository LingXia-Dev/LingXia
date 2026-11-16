export const contextMenuParts = {
  target: 'view',
  menu: 'view',
  scrim: 'button',
  sheet: 'view',
  title: 'text',
} as const;

/**
 * DropdownMenu's menu, opened where it was asked for. From a finger it docks to the bottom edge
 * like a sheet, over a scrim, so it always fits and sits under the thumb; from a mouse or the
 * keyboard it opens at the point.
 */
export const contextMenuRecipe = {
  target: {
    base: '',
  },
  /** Placed in its layer, never `fixed`: a fixed panel is not pressed through its own backdrop. */
  menu: {
    base: 'absolute transition duration-fast ease-standard starting:opacity-0 exit:opacity-0',
    point: 'min-w-48 p-1 gap-0.5 rounded-container border border-text/12 bg-surface-overlay shadow-raised motion-safe:starting:scale-95',
    /** Docked, it covers the page: the scrim, and the sheet at its foot. Without `m-0` a popover centres itself. */
    docked: 'inset-0 m-0 justify-end',
  },
  /** Dims the page, as under every sheet, and takes the press that closes the menu. */
  scrim: {
    base: 'absolute inset-0 bg-scrim',
  },
  sheet: {
    base: 'mx-2 p-2 gap-1 rounded-dialog border border-text/12 bg-surface-overlay shadow-modal transition duration-fast ease-standard motion-safe:starting:translate-y-4',
  },
  /** Docked, the menu says what it is for; at a point the thing under the pointer says it. */
  title: {
    base: 'px-3 pt-1.5 pb-1 text-footnote font-medium text-muted',
  },
} as const;
