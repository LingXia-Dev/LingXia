export const contextMenuParts = {
  target: 'view',
  menu: 'view',
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
  menu: {
    base: 'transition duration-fast ease-standard starting:opacity-0 exit:opacity-0',
    /** Placed in its layer, never `fixed`: a fixed box in a top layer stops at the bottom safe area. */
    point: 'absolute min-w-48 max-w-[90vw] p-1 gap-0.5 rounded-container border border-text/12 bg-surface-overlay shadow-raised',
    /** It slides 4 px out from the point, downward or, in the lower part of the page, upward. */
    from: { below: 'motion-safe:starting:-translate-y-1', above: 'motion-safe:starting:translate-y-1' },
    /**
     * Docked, the menu is an edge sheet: full width, square at the foot, over a scrim that takes the
     * tap that closes it. It runs to the edge; the layer pads it by the home indicator's inset.
     */
    docked: 'mt-auto mb-0 mx-0 w-full px-2 pt-2 pb-2 gap-1 rounded-t-dialog rounded-b-none border border-b-0 border-text/12 bg-surface-overlay shadow-modal backdrop:bg-scrim motion-safe:starting:translate-y-4 motion-safe:exit:translate-y-4',
  },
  /** Docked, the menu says what it is for; at a point the thing under the pointer says it. */
  title: {
    base: 'px-3 pt-1.5 pb-1 text-footnote font-medium text-muted',
  },
} as const;
