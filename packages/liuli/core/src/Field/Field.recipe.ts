export const fieldParts = {
  root: 'view',
  label: 'text',
  required: 'span',
  description: 'text',
  status: 'view',
  statusIcon: 'vector',
  statusText: 'text',
} as const;

/**
 * Field chrome around one control. Status text stays in the text role: status roles reach 3:1,
 * enough for the icon but not for small text, so the icon carries the colour.
 */
export const fieldRecipe = {
  root: {
    base: 'gap-1.5',
  },
  label: {
    base: 'font-medium text-text',
    size: { sm: 'text-xs', md: 'text-sm', lg: 'text-base' },
    disabled: 'text-faint',
  },
  required: {
    base: 'text-danger',
  },
  description: {
    base: 'text-xs text-muted',
  },
  status: {
    empty: 'sr-only',
    shown: 'flex-row items-start gap-1.5',
  },
  statusIcon: {
    base: 'size-4 shrink-0',
    type: { error: 'text-danger', warning: 'text-warning', success: 'text-success' },
  },
  statusText: {
    base: 'grow basis-0 text-xs text-text',
  },
} as const;
