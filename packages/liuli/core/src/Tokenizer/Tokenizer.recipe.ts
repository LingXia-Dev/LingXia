import type { FieldSize, FieldStatusType } from '../Field/Field';

export const tokenizerParts = {
  box: 'view',
  editor: 'input',
} as const;

/** MultiSelect's box, with the editor after the last token taking the rest of the line and wrapping when it is short. */
export const tokenizerRecipe = {
  box: {
    base: 'flex-row flex-wrap items-center gap-1.5 p-1 ps-1.5 rounded-control border border-line-strong bg-surface transition duration-fast ease-standard hover:border-muted focus-within:border-accent focus-within:inset-ring-1 focus-within:inset-ring-accent',
    size: { sm: 'min-h-control-sm', md: 'min-h-control-md', lg: 'min-h-control-lg' } satisfies Record<FieldSize, string>,
    status: { error: 'border-danger', warning: 'border-warning', success: 'border-line-strong' } satisfies Record<FieldStatusType, string>,
    disabled: 'bg-text/5 border-text/8',
  },
  editor: {
    base: 'grow basis-24 min-w-24 self-stretch px-1.5 bg-transparent text-text placeholder:text-muted disabled:text-faint',
    size: { sm: 'text-footnote', md: 'text-body', lg: 'text-callout' } satisfies Record<FieldSize, string>,
  },
} as const;
