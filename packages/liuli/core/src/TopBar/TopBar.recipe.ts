export const topBarParts = {
  root: 'view',
  start: 'view',
  heading: 'text',
  center: 'view',
  end: 'view',
} as const;

/**
 * Three regions: the start and the end share the room left over equally, so the centre stays at the
 * bar's middle while everything fits; past that the bar wraps rather than squeeze the centre.
 */
export const topBarRecipe = {
  root: {
    base: 'flex-row flex-wrap items-center gap-x-2 min-h-[44px] px-3 py-1 border-b border-text/8 bg-surface shrink-0',
  },
  start: {
    base: 'grow basis-0 min-w-0 flex-row items-center gap-1',
  },
  heading: {
    base: 'shrink min-w-0 text-body font-semibold text-text truncate',
  },
  center: {
    base: 'shrink-0 flex-row items-center justify-center',
  },
  end: {
    base: 'grow basis-0 flex-row items-center justify-end gap-1',
  },
} as const;
