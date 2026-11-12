export const itemParts = {
  root: 'view',
  start: 'view',
  text: 'view',
  label: 'text',
  description: 'text',
  end: 'view',
  chevron: 'vector',
} as const;

/** List row classes by part. A pressable row lands on a `button`, a plain one on a `view`. */
export const itemRecipe = {
  root: {
    base: 'flex-row items-center gap-3 px-4 py-2.5 min-h-control-lg self-stretch text-start',
    pressable: 'transition-colors duration-150 hover:bg-surface-sunken active:bg-surface-sunken disabled:opacity-40',
  },
  start: {
    base: 'shrink-0 items-center justify-center text-muted',
  },
  text: {
    base: 'grow shrink basis-0 gap-0.5',
  },
  label: {
    base: 'text-base text-text',
  },
  description: {
    base: 'text-sm text-muted',
  },
  end: {
    base: 'shrink-0 flex-row items-center gap-2 text-muted',
  },
  chevron: {
    base: 'size-5 shrink-0 text-faint rtl:-scale-x-100',
  },
} as const;
