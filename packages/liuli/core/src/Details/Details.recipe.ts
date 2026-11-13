export const detailsParts = {
  group: 'view',
  root: 'view',
  summary: 'button',
  label: 'text',
  chevron: 'vector',
  body: 'view',
} as const;

/** A disclosure row: the summary is the press target, the chevron turns as it opens. */
export const detailsRecipe = {
  group: {
    base: 'overflow-hidden rounded-container border border-text/10 bg-surface divide-y divide-text/8',
  },
  root: {
    base: '',
  },
  summary: {
    base: 'flex-row items-center gap-3 min-h-control-lg py-2.5 text-start transition-colors duration-150 ease-out hover:bg-text/4 active:bg-text/7 focus-visible:bg-text/7',
    /** Alone, the hover wash reaches past the text, which stays in line with the content around it. */
    alone: '-mx-3 px-3 rounded-control',
    inGroup: 'px-4',
  },
  label: {
    base: 'grow shrink basis-0 text-body font-medium text-text',
  },
  chevron: {
    base: 'size-5 shrink-0 text-muted transition-transform duration-200 ease-out',
    open: '-rotate-180',
  },
  body: {
    base: 'pb-4 gap-2 transition duration-200 ease-out starting:opacity-0 motion-safe:starting:-translate-y-1',
    inGroup: 'px-4',
  },
} as const;
