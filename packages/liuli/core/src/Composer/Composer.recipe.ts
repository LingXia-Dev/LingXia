export const composerParts = {
  root: 'view',
  surface: 'view',
  attachments: 'view',
  start: 'view',
  field: 'view',
  editor: 'textarea',
  send: 'IconButton',
  measure: 'view',
  line: 'text',
  reason: 'text',
} as const;

/**
 * One surface: its border is the field's edge, and focus turns all of it to a 2 px accent edge. In one
 * row it is start, text, send; stacked, the text takes the first row whole and the buttons the
 * row under it, at its two ends. The same three children in the same order either way, placed
 * by the grid, so the editor keeps its caret through the change.
 */
export const composerRecipe = {
  root: {
    base: 'self-stretch gap-1.5',
  },
  surface: {
    base: 'grid grid-cols-[auto_1fr_auto] items-center gap-x-1 gap-y-1 p-1.5 rounded-container border border-line-strong bg-surface transition-size duration-base ease-standard focus-within:border-accent focus-within:inset-ring-1 focus-within:inset-ring-accent',
    disabled: 'bg-text/5 border-text/8',
  },
  /** Over the text, the whole width. */
  attachments: {
    base: 'col-span-3 flex-row flex-wrap gap-1.5 px-1.5 pt-1.5',
  },
  start: {
    base: 'col-start-1 flex-row items-center',
    stacked: 'order-2',
  },
  /** The text's inset is here, not on the editor, so the editor's height is a whole number of lines. */
  field: {
    base: 'min-w-0 px-2',
    row: 'col-start-2',
    /** With nothing before it the text starts at the edge, where it starts when stacked. */
    rowAlone: 'col-start-1 col-span-2',
    stacked: 'order-1 col-span-3 pt-1.5',
  },
  editor: {
    base: 'self-stretch p-0 bg-transparent text-body text-text placeholder:text-muted disabled:text-faint',
  },
  send: {
    // Concentric with the box: its 14 px container corner less the 6 px padding.
    base: 'col-start-3 rounded-[8px]',
    stacked: 'order-3',
  },
  measure: {
    base: 'absolute size-0 overflow-hidden opacity-0 flex-row',
    line: 'shrink-0 whitespace-nowrap text-body',
  },
  /** Why it is disabled, under the composer. */
  reason: {
    base: 'text-footnote text-muted',
  },
} as const;
