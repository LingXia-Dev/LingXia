export const treeListParts = {
  root: 'view',
  row: 'button',
  chevron: 'vector',
  spacer: 'view',
  icon: 'vector',
  label: 'text',
  end: 'text',
} as const;

/** Rows like NavList's, set in by their depth; a branch carries a chevron that turns when it opens. */
export const treeListRecipe = {
  root: {
    base: 'gap-0.5',
  },
  row: {
    base: 'flex-row items-center gap-1.5 min-h-9 pe-3 rounded-control text-start transition-colors duration-fast ease-standard focus-visible:inset-ring-2 focus-visible:inset-ring-focus disabled:opacity-40',
    idle: 'hover:bg-text/4 active:bg-text/7 focus-visible:bg-text/7',
    selected: 'bg-accent-soft hover:bg-accent/16 active:bg-accent/20 focus-visible:bg-accent/20',
    /** 20 px a level, to six levels; deeper rows stay at the sixth. */
    level: { 1: 'ps-2', 2: 'ps-7', 3: 'ps-12', 4: 'ps-17', 5: 'ps-22', 6: 'ps-27' } satisfies Record<1 | 2 | 3 | 4 | 5 | 6, string>,
  },
  chevron: {
    base: 'size-4 shrink-0 text-muted transition-transform duration-base ease-standard',
    /** Points along the line while the branch is closed, mirrored for right-to-left text. */
    closed: '-rotate-90 rtl:rotate-90',
  },
  /** A leaf keeps the chevron's width, so labels line up down a level. */
  spacer: {
    base: 'size-4 shrink-0',
  },
  icon: {
    base: 'size-4 shrink-0 text-muted',
  },
  label: {
    base: 'grow shrink basis-0 truncate text-body text-text',
    selected: 'font-medium',
  },
  end: {
    base: 'shrink-0 text-footnote text-muted tabular-nums',
  },
} as const;
