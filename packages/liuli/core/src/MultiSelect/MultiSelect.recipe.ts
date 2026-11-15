import type { FieldSize, FieldStatusType } from '../Field/Field';

export const multiSelectParts = {
  box: 'view',
  placeholder: 'text',
  add: 'DropdownMenu',
} as const;

/** The same box as TextInput, grown to hold tokens that wrap; the menu that adds sits after the last one. */
export const multiSelectRecipe = {
  box: {
    base: 'flex-row flex-wrap items-center gap-1.5 p-1 ps-1.5 rounded-control border border-line-strong bg-surface',
    size: { sm: 'min-h-control-sm', md: 'min-h-control-md', lg: 'min-h-control-lg' } satisfies Record<FieldSize, string>,
    status: { error: 'border-danger', warning: 'border-warning', success: 'border-line-strong' } satisfies Record<FieldStatusType, string>,
    disabled: 'bg-text/5 border-text/8',
  },
  placeholder: {
    base: 'grow basis-0 min-w-0 truncate ps-1.5 text-body text-muted',
  },
  add: {
    base: 'ms-auto',
  },
} as const;
