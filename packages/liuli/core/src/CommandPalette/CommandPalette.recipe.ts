export const commandPaletteParts = {
  panel: 'dialog',
  search: 'view',
  searchIcon: 'vector',
  input: 'input',
  spinner: 'view',
  list: 'scroll',
  group: 'view',
  groupLabel: 'text',
  option: 'button',
  optionIcon: 'vector',
  optionText: 'view',
  optionLabel: 'text',
  optionDescription: 'text',
  optionEnd: 'text',
  optionShortcut: 'view',
  note: 'text',
} as const;

/**
 * A dialog near the top of the screen: a search line over one list of results. On a phone it
 * spans the width from the top edge, so the results scroll above the keyboard.
 */
export const commandPaletteRecipe = {
  panel: {
    base: 'mb-auto p-0 gap-0 overflow-hidden border-line bg-surface-overlay shadow-modal transition duration-fast ease-standard starting:opacity-0 exit:opacity-0',
    regular: 'w-[90%] max-w-140 max-h-[70vh] rounded-dialog border',
    compact: 'mt-0 w-full max-h-[92vh] rounded-none rounded-b-dialog border-b',
  },
  search: {
    base: 'flex-row items-center gap-3 min-h-control-lg px-4 border-b border-line',
  },
  searchIcon: {
    base: 'size-5 shrink-0 text-muted',
  },
  input: {
    base: 'grow basis-0 min-w-0 self-stretch px-0 bg-transparent text-body text-text placeholder:text-muted',
  },
  spinner: {
    base: 'size-4 shrink-0 text-muted motion-safe:animate-spin',
  },
  list: {
    base: 'shrink min-h-0 p-1.5 gap-0.5 overscroll-contain',
  },
  group: {
    base: 'gap-0.5',
  },
  groupLabel: {
    base: 'px-2.5 pt-2 pb-1 text-caption font-medium text-muted',
  },
  option: {
    base: 'flex-row items-center gap-3 min-h-control-lg px-2.5 rounded-control text-start text-text hover:bg-text/5 active:bg-text/8 disabled:opacity-40',
    /** The row the arrow keys are on, while focus stays in the search line. */
    active: 'bg-text/8',
  },
  optionIcon: {
    base: 'size-5 shrink-0 text-muted',
  },
  optionText: {
    base: 'grow basis-0 min-w-0',
  },
  optionLabel: {
    base: 'truncate text-body',
  },
  optionDescription: {
    base: 'truncate text-footnote text-muted',
  },
  optionEnd: {
    base: 'shrink-0 text-footnote text-muted tabular-nums',
  },
  optionShortcut: {
    base: 'shrink-0',
  },
  note: {
    base: 'px-4 py-6 text-center text-footnote text-muted',
  },
} as const;
