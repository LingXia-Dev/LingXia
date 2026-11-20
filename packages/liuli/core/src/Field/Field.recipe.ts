import type { FieldSize, FieldStatusType } from './Field';

export const fieldParts = {
  root: 'view',
  labelRow: 'view',
  info: 'InfoTip',
  label: 'text',
  required: 'span',
  optional: 'text',
  description: 'text',
  status: 'view',
  statusLine: 'view',
  statusGhost: 'text',
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
  labelRow: {
    base: 'flex-row items-baseline gap-1.5',
  },
  /** The 32 px info button, centred on the label's first line and no taller than it, so a wrapped label keeps it at its start. */
  info: {
    base: 'self-start',
    size: { sm: '-my-2', md: '-my-[7px]', lg: '-my-[5px]' } satisfies Record<FieldSize, string>,
  },
  required: {
    base: 'text-danger',
  },
  optional: {
    base: 'pb-0.5 text-muted',
    size: { sm: 'text-caption', md: 'text-footnote', lg: 'text-footnote' } satisfies Record<FieldSize, string>,
  },
  description: {
    base: 'text-footnote text-muted',
  },
  status: {
    empty: 'sr-only',
    shown: 'flex-row items-start gap-1.5',
  },
  /** The message's first line box, holding the icon centred on it at any text size. */
  statusLine: {
    base: 'shrink-0 flex-row items-center text-footnote',
  },
  /** An empty run in the message's type: it gives the line its height. */
  statusGhost: {
    base: 'opacity-0',
  },
  statusIcon: {
    // In the message's type (its line box sets it), so the icon grows with the text, capped by the View at 1.5×.
    base: 'size-[1.23em] shrink-0',
    type: { error: 'text-danger', warning: 'text-warning', success: 'text-success' } satisfies Record<FieldStatusType, string>,
  },
  statusText: {
    base: 'grow basis-0 text-footnote text-text',
  },
} as const;
