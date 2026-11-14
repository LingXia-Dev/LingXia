import type { FieldSize, FieldStatusType } from '../Field/Field';

export const timeInputParts = { box: 'view', input: 'input', icon: 'vector' } as const;

/**
 * TextInput's box: a 3:1 boundary that takes the focus colour, the editor borderless inside it. The
 * mark at the end says a tap opens a picker, not a keyboard.
 */
export const timeInputRecipe = {
  box: {
    base: 'flex-row items-center gap-2 px-3 rounded-control border border-line-strong bg-surface transition duration-150 ease-out focus-within:border-accent focus-within:ring-3 focus-within:ring-accent/20',
    size: { sm: 'min-h-control-sm', md: 'min-h-control-md', lg: 'min-h-control-lg' } satisfies Record<FieldSize, string>,
    status: { error: 'border-danger', warning: 'border-warning', success: 'border-line-strong' } satisfies Record<FieldStatusType, string>,
    disabled: 'bg-text/5 border-text/8',
  },
  input: {
    /** A long placeholder ends in an ellipsis, the same as TextInput's. */
    base: 'grow basis-0 truncate self-stretch px-0 bg-transparent text-text tabular-nums placeholder:text-muted disabled:text-faint',
    size: { sm: 'text-footnote', md: 'text-body', lg: 'text-base' } satisfies Record<FieldSize, string>,
    /** The editor never narrows below `HH:MM` in its own tabular digits. */
    width: 'min-w-[5ch]',
    /** Once the value carries seconds, `HH:MM:SS`. */
    widthSeconds: 'min-w-[8ch]',
  },
  icon: {
    base: 'size-4 shrink-0 text-muted',
  },
} as const;
