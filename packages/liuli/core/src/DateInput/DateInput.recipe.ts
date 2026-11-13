import type { FieldSize, FieldStatusType } from '../Field/Field';

export const dateInputParts = { box: 'view', input: 'input', icon: 'vector' } as const;

/**
 * TextInput's box: a 3:1 boundary that takes the focus colour, the editor borderless inside it. The
 * mark at the end says a tap opens a picker, not a keyboard.
 */
export const dateInputRecipe = {
  box: {
    base: 'flex-row items-center gap-2 px-3 rounded-control border border-line-strong bg-surface transition duration-150 ease-out focus-within:border-accent focus-within:ring-3 focus-within:ring-accent/20',
    size: { sm: 'min-h-control-sm', md: 'min-h-control-md', lg: 'min-h-control-lg' } satisfies Record<FieldSize, string>,
    status: { error: 'border-danger', warning: 'border-warning', success: 'border-line-strong' } satisfies Record<FieldStatusType, string>,
    disabled: 'bg-text/5 border-line',
  },
  input: {
    base: 'grow basis-0 min-w-0 self-stretch px-0 bg-transparent text-text tabular-nums placeholder:text-muted disabled:text-faint',
    size: { sm: 'text-footnote', md: 'text-body', lg: 'text-base' } satisfies Record<FieldSize, string>,
  },
  icon: {
    base: 'size-4 shrink-0 text-muted',
  },
} as const;
