export const splitParts = {
  root: 'view',
  primary: 'view',
  aside: 'view',
  hingeGap: 'view',
  handle: 'view',
  handleLine: 'view',
} as const;

export const splitRecipe = {
  root: {
    base: 'grow min-h-0 bg-canvas',
    row: 'flex-row',
    column: 'flex-col',
  },
  primary: {
    base: 'grow shrink basis-0 min-w-0 min-h-0 flex-col',
    tabletop: 'grow shrink basis-0 min-w-0 min-h-0 flex-col border-b border-line',
  },
  aside: {
    base: 'min-w-0 flex-col bg-canvas',
    side: 'relative shrink-0 border-s border-line',
    stacked: 'grow shrink basis-0 min-h-0 border-t border-line',
    tabletop: 'shrink-0 min-h-0',
  },
  hingeGap: {
    base: 'shrink-0 bg-canvas',
  },
  /** A strip over the aside's edge, wider than the rule it lights, so a pointer finds it. */
  handle: {
    base: 'absolute inset-y-0 start-0 w-3 -ms-2 items-center',
  },
  /** The edge lights while a pointer is over it, and takes the accent while held. */
  handleLine: {
    base: 'h-full w-0.5 ms-px transition-colors duration-fast ease-standard',
    state: { rest: 'bg-transparent', hover: 'bg-line-strong', held: 'bg-accent' },
  },
} as const;
