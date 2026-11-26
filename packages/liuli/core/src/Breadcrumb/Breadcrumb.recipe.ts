export const breadcrumbParts = {
  root: 'view',
  item: 'view',
  link: 'button',
  label: 'text',
  separator: 'vector',
} as const;

/**
 * A quiet line of levels: the ways back are muted and firm up under a pointer, the current page
 * is in the text colour and a heavier weight. A long trail wraps by whole levels.
 */
export const breadcrumbRecipe = {
  root: {
    base: 'flex-row flex-wrap items-center gap-y-1 self-stretch',
  },
  item: {
    base: 'flex-row items-center min-w-0',
  },
  link: {
    base: 'min-h-8 px-1.5 -mx-0.5 flex-row items-center rounded-control transition-colors duration-fast ease-standard hover:bg-text/5 active:bg-text/8 focus-visible:ring-2 focus-visible:ring-focus',
  },
  label: {
    base: 'text-footnote max-w-48 truncate',
    link: 'text-muted',
    plain: 'px-1 text-muted',
    current: 'px-1 font-medium text-text',
  },
  separator: {
    base: 'size-4 shrink-0 mx-0.5 text-faint',
  },
  more: {
    // The button keeps its 32 px box for the finger and gives up the room beside its dots, so the chevrons sit evenly.
    base: '-mx-1',
  },
} as const;
