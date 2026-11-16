export const listInputParts = {
  root: 'view',
  list: 'view',
  row: 'view',
  controls: 'view',
  add: 'view',
} as const;

/** Rows of the app's own controls, each with a remove button at its end, over a button that adds one. */
export const listInputRecipe = {
  root: {
    base: 'gap-2',
  },
  list: {
    base: 'gap-2',
  },
  /** The remove button sits level with the row's first line of controls. */
  row: {
    base: 'flex-row items-start gap-1',
  },
  controls: {
    base: 'grow shrink basis-0 min-w-0',
  },
  add: {
    base: 'flex-row',
  },
} as const;
