import type { FieldSize, FieldStatusType } from '../Field/Field';

export const textAreaParts = { box: 'view', editor: 'textarea', count: 'text' } as const;

/**
 * The same box as TextInput: a 3:1 boundary, the focus colour through `focus-within:`, the status
 * colour otherwise. The editor fills the box and keeps its own inset, so the first line sits where a
 * TextInput's text does.
 */
export const textAreaRecipe = {
  box: {
    base: 'rounded-control border border-line-strong bg-surface transition duration-150 ease-out focus-within:border-accent focus-within:ring-3 focus-within:ring-accent/20',
    status: { error: 'border-danger', warning: 'border-warning', success: 'border-line-strong' } satisfies Record<FieldStatusType, string>,
    disabled: 'bg-text/5 border-text/8',
  },
  /** Inside the box, under the text at the end: it turns to the text colour when the limit is reached. */
  count: {
    base: 'self-end px-3 pb-2 text-caption text-muted tabular-nums',
    full: 'text-text font-medium',
  },
  editor: {
    base: 'self-stretch px-3 py-2 bg-transparent text-text placeholder:text-muted disabled:text-faint',
    /** Matches TextInput's text at each size. */
    size: { sm: 'text-footnote', md: 'text-body', lg: 'text-base' } satisfies Record<FieldSize, string>,
  },
} as const;
