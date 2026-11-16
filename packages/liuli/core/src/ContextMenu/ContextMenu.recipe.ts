export const contextMenuParts = {
  target: 'view',
  menu: 'view',
  title: 'text',
} as const;

/**
 * DropdownMenu's menu, opened where it was asked for. From a finger it docks to the bottom edge
 * like a sheet, so it always fits and sits under the thumb; from a mouse or the keyboard it opens
 * at the point.
 */
export const contextMenuRecipe = {
  target: {
    base: '',
  },
  menu: {
    base: 'fixed p-1 gap-0.5 rounded-container border border-text/12 bg-surface-overlay shadow-raised transition duration-fast ease-standard starting:opacity-0 exit:opacity-0',
    point: 'min-w-48 motion-safe:starting:scale-95',
    docked: 'inset-x-2 p-2 gap-1 rounded-dialog shadow-modal motion-safe:starting:translate-y-4 motion-safe:exit:translate-y-4',
  },
  /** Docked, the menu says what it is for; at a point the thing under the pointer says it. */
  title: {
    base: 'px-3 pt-1.5 pb-1 text-footnote font-medium text-muted',
  },
} as const;
