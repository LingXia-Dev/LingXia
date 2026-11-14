export const navListParts = {
  root: 'view',
  group: 'button',
  groupLabel: 'text',
  groupChevron: 'vector',
  item: 'button',
  label: 'text',
  end: 'view',
} as const;

/**
 * Sidebar rows: quiet until hovered, the current one on an accent wash. Rows are rounded inside the
 * sidebar's padding rather than ruled, so a long list reads as one column.
 */
export const navListRecipe = {
  root: {
    base: 'gap-0.5',
  },
  group: {
    base: 'flex-row items-center gap-2 min-h-8 px-3 mt-3 first:mt-0 rounded-control text-start transition-colors duration-150 ease-out hover:bg-text/4 active:bg-text/7 focus-visible:bg-text/7 focus-visible:inset-ring-2 focus-visible:inset-ring-focus',
  },
  groupLabel: {
    base: 'grow shrink basis-0 text-footnote font-semibold text-muted',
  },
  groupChevron: {
    base: 'size-4 shrink-0 text-muted transition-transform duration-200 ease-out',
    /** Points along the line while the group is closed, mirrored for right-to-left text. */
    closed: '-rotate-90 rtl:rotate-90',
  },
  item: {
    /** Keyboard focus draws the focus ring inside the row, so the sidebar's edge never clips it. */
    base: 'flex-row items-center gap-2 min-h-9 px-3 rounded-control text-start transition-colors duration-150 ease-out focus-visible:inset-ring-2 focus-visible:inset-ring-focus disabled:opacity-40',
    idle: 'hover:bg-text/4 active:bg-text/7 focus-visible:bg-text/7',
    /** Item's selected wash, deepening on hover, press and focus. */
    selected: 'bg-accent-soft hover:bg-accent/16 active:bg-accent/20 focus-visible:bg-accent/20',
  },
  label: {
    base: 'grow shrink basis-0 truncate text-body text-text',
    selected: 'font-medium',
  },
  end: {
    base: 'shrink-0 text-footnote text-muted',
  },
} as const;
