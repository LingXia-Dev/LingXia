import type { FieldStatusType } from '../Field/Field';

export const fileInputParts = {
  root: 'view',
  pick: 'button',
  pickIcon: 'vector',
  pickLabel: 'text',
  hint: 'text',
  list: 'view',
  file: 'view',
  fileIcon: 'vector',
  fileText: 'view',
  name: 'text',
  detail: 'text',
} as const;

/** A wide, quiet button that opens the app\'s picker, then one row per chosen file. */
export const fileInputRecipe = {
  root: {
    base: 'gap-2',
  },
  pick: {
    base: 'items-center justify-center gap-1 px-4 py-5 rounded-control border border-line-strong bg-surface-sunken text-center transition-colors duration-fast ease-standard hover:bg-text/5 active:bg-text/8 focus-visible:ring-2 focus-visible:ring-focus focus-visible:ring-offset-2 disabled:opacity-40',
    status: { error: 'border-danger', warning: 'border-warning', success: 'border-line-strong' } satisfies Record<FieldStatusType, string>,
  },
  pickIcon: {
    base: 'size-5 text-muted',
  },
  pickLabel: {
    base: 'text-body font-medium text-text',
  },
  hint: {
    base: 'text-footnote text-muted',
  },
  list: {
    base: 'gap-1.5',
  },
  file: {
    base: 'flex-row items-center gap-3 min-h-12 ps-3 pe-1 rounded-control border border-text/12 bg-surface',
    failed: 'border-danger',
  },
  fileIcon: {
    base: 'size-5 shrink-0 text-muted',
  },
  fileText: {
    base: 'grow shrink basis-0 min-w-0 gap-0.5 py-1.5',
  },
  name: {
    base: 'truncate text-footnote font-medium text-text',
  },
  detail: {
    base: 'text-caption text-muted tabular-nums',
    failed: 'text-danger',
  },
} as const;
