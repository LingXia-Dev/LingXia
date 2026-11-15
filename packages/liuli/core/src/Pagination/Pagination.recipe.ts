export const paginationParts = {
  root: 'view',
  pages: 'view',
  page: 'button',
  pageLabel: 'text',
  gap: 'text',
  position: 'text',
} as const;

/**
 * Ghost arrows around either a row of page numbers, the current one on a soft accent fill, or
 * the position in tabular figures. Nothing moves as the page changes: every number keeps its width.
 */
export const paginationRecipe = {
  root: {
    // As wide as its row, so it can tell whether the page numbers fit.
    base: 'flex-row items-center gap-1 self-stretch',
  },
  pages: {
    base: 'flex-row items-center gap-1',
  },
  page: {
    base: 'h-control-md min-w-control-md px-2 items-center justify-center rounded-control transition-colors duration-150 ease-out focus-visible:ring-2 focus-visible:ring-focus focus-visible:ring-offset-2',
    idle: 'hover:bg-text/5 active:bg-text/8',
    current: 'bg-accent-soft',
  },
  pageLabel: {
    base: 'text-body text-text tabular-nums',
    current: 'font-semibold text-on-accent-soft',
  },
  gap: {
    base: 'w-6 text-center text-body text-muted',
  },
  position: {
    base: 'min-w-16 px-2 text-center text-body text-text tabular-nums',
  },
} as const;
