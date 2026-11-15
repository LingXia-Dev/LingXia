export const composerParts = {
  root: 'view',
  row: 'view',
  reason: 'text',
  start: 'view',
  field: 'TextArea',
  send: 'IconButton',
} as const;

/** The field grows upward from one line; what sits beside it stays on its last line. */
export const composerRecipe = {
  root: {
    base: 'self-stretch gap-1.5',
  },
  row: {
    base: 'flex-row items-end gap-2',
  },
  /** Why it is disabled, under the whole row. */
  reason: {
    base: 'text-footnote text-muted',
  },
  start: {
    base: 'shrink-0 flex-row items-center',
  },
  field: {
    base: 'grow basis-0 min-w-0',
  },
  /** On the field's last line, not at its middle. */
  send: {
    base: 'shrink-0 self-end',
  },
} as const;
