import type { FieldSize, FieldStatusType } from './Field';

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
  /** 8 from label to control (gap plus the label's own bottom space), 6 from control to help. */
  root: {
    base: 'gap-1.5',
  },
  label: {
    base: 'pb-0.5 font-medium text-text',
    size: { sm: 'text-caption', md: 'text-footnote', lg: 'text-body' } satisfies Record<FieldSize, string>,
    disabled: 'text-faint',
  },
  required: {
    base: 'text-danger',
  },
  description: {
    base: 'text-footnote text-muted',
  },
  status: {
    empty: 'sr-only',
    shown: 'flex-row items-start gap-1.5',
  },
  statusIcon: {
    base: 'size-4 shrink-0 mt-px',
    type: { error: 'text-danger', warning: 'text-warning', success: 'text-success' } satisfies Record<FieldStatusType, string>,
  },
  statusText: {
    base: 'grow basis-0 text-footnote text-text',
  },
} as const;
