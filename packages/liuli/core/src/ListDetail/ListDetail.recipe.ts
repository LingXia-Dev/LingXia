export const listDetailParts = {
  root: 'view',
  listPane: 'view',
  detailPane: 'view',
  compactHeader: 'view',
  compactTitle: 'text',
} as const;

export const listDetailRecipe = {
  root: {
    base: 'grow min-h-0 bg-canvas',
    regular: 'flex-row',
    compact: 'overflow-hidden',
  },
  listPane: {
    base: 'min-h-0 flex-col bg-canvas',
    regular: 'shrink-0 border-e border-text/8',
    compact: 'grow shrink basis-0',
    hidden: 'hidden',
  },
  detailPane: {
    base: 'grow shrink basis-0 min-w-0 min-h-0 flex-col bg-canvas',
  },
  compactHeader: {
    base: 'flex-row items-center gap-2 ps-2 pe-4 py-2 border-b border-text/8 shrink-0',
  },
  compactTitle: {
    base: 'text-body font-semibold text-text truncate',
  },
} as const;
