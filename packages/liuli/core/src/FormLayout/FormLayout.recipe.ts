export const formLayoutParts = {
  root: 'view',
  fields: 'view',
  span: 'view',
  actions: 'view',
} as const;

/** One distance between fields in both directions, and the actions set apart at the end edge. */
export const formLayoutRecipe = {
  root: {
    base: 'self-stretch gap-6',
  },
  fields: {
    base: 'self-stretch gap-4',
    columns: { 1: '', 2: 'grid grid-cols-2 items-start' } satisfies Record<1 | 2, string>,
  },
  span: {
    base: 'self-stretch min-w-0',
    wide: 'col-span-2',
  },
  actions: {
    base: 'flex-row flex-wrap items-center justify-end gap-2',
  },
} as const;
