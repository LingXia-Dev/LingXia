export const overflowListParts = { root: 'view', item: 'view', overflow: 'view' } as const;

/**
 * One line that never wraps: the items that fit, then whatever stands for the rest. Until an item
 * is measured it is laid out with the others and clipped, so nothing flashes on a second line.
 */
export const overflowListRecipe = {
  root: {
    base: 'flex-row items-center gap-2 min-w-0 overflow-hidden',
  },
  item: {
    base: 'shrink-0',
  },
  overflow: {
    base: 'shrink-0',
    /** Stands in at its widest while the line is measured, unseen, so a line that fits never flashes it. */
    measuring: 'opacity-0',
  },
} as const;
