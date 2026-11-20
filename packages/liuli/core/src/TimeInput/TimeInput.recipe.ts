import type { FieldSize, FieldStatusType } from '../Field/Field';

export const timeInputParts = { box: 'view', input: 'input', icon: 'vector' } as const;

/**
 * TextInput's box: a 3:1 boundary that takes the focus colour, the editor borderless inside it. The
 * mark at the end says a tap opens a picker, not a keyboard.
 */
export const timeInputRecipe = {
  box: {
    base: 'flex-row items-center gap-2 px-3 rounded-control border border-line-strong bg-surface transition duration-fast ease-standard focus-within:border-accent focus-within:inset-ring-1 focus-within:inset-ring-accent',
    size: { sm: 'min-h-control-sm', md: 'min-h-control-md', lg: 'min-h-control-lg' } satisfies Record<FieldSize, string>,
    status: { error: 'border-danger', warning: 'border-warning', success: 'border-line-strong' } satisfies Record<FieldStatusType, string>,
    disabled: 'bg-text/5 border-text/8',
  },
  input: {
    /** A long placeholder ends in an ellipsis, the same as TextInput's. */
    base: 'grow basis-0 truncate self-stretch px-0 bg-transparent text-text tabular-nums placeholder:text-muted disabled:text-faint',
    /** The View keeps a time field as wide as its widest value in the user's clock, so it needs no minimum of its own. */
    size: { sm: 'text-footnote', md: 'text-body', lg: 'text-callout' } satisfies Record<FieldSize, string>,
  },
  icon: {
    base: 'size-4 shrink-0 text-muted',
  },
} as const;
