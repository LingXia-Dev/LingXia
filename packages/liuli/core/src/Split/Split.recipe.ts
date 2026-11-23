export const splitParts = {
  root: 'view',
  primary: 'view',
  aside: 'view',
  hingeGap: 'view',
} as const;

export const splitRecipe = {
  root: {
    base: 'grow min-h-0 bg-canvas',
    row: 'flex-row',
    column: 'flex-col',
  },
  primary: {
    base: 'grow shrink basis-0 min-w-0 min-h-0 flex-col',
    tabletop: 'grow shrink basis-0 min-w-0 min-h-0 flex-col border-b border-text/8',
  },
  aside: {
    base: 'min-w-0 flex-col bg-canvas',
    side: 'shrink-0 border-s border-text/8',
    stacked: 'grow shrink basis-0 min-h-0 border-t border-text/8',
    tabletop: 'shrink-0 min-h-0',
  },
  hingeGap: {
    base: 'shrink-0 bg-canvas',
  },
} as const;
