export const composerParts = {
  root: 'view',
  surface: 'view',
  attachments: 'view',
  start: 'view',
  field: 'view',
  editor: 'textarea',
  end: 'view',
  send: 'IconButton',
  measure: 'view',
  line: 'text',
  reason: 'text',
  suggestions: 'view',
  suggestionList: 'view',
  suggestion: 'button',
  suggestionLabel: 'text',
  suggestionDescription: 'text',
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
  /** The app's end actions and the send button, together in the send button's place. */
  end: {
    base: 'col-start-3 flex-row items-center gap-1',
    stacked: 'order-3',
  },
  send: {
    // Concentric with the box: the pack's container corner less the 6 px padding.
    base: 'rounded-[calc(var(--radius-container)-6px)]',
    /** Alone, it takes the end place itself. */
    alone: 'col-start-3',
    stacked: 'order-3',
  },
  measure: {
    base: 'absolute size-0 overflow-hidden flex-row',
    line: 'shrink-0 whitespace-nowrap text-body',
  },
  /** Why it is disabled, under the composer. */
  reason: {
    base: 'text-footnote text-muted',
  },
  /** A trigger's list, over the text at the caret: a menu's panel. */
  suggestions: {
    base: 'w-64 mb-1.5 p-1 rounded-container border border-line bg-surface-overlay shadow-raised transition duration-fast ease-standard starting:opacity-0 exit:opacity-0',
  },
  suggestionList: {
    base: 'gap-0.5',
  },
  /** The active one is marked as a hovered menu item is, and a pointer can press any. */
  suggestion: {
    base: 'flex-row items-baseline gap-2 min-h-9 px-2.5 py-1.5 rounded-control text-start hover:bg-text/5 active:bg-text/8',
    active: 'bg-text/8',
  },
  suggestionLabel: {
    base: 'shrink-0 text-body text-text',
  },
  suggestionDescription: {
    base: 'shrink min-w-0 text-footnote text-muted truncate',
  },
} as const;
