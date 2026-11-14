import type { EmptyStateSize } from './EmptyState';

export const emptyStateParts = { root: 'view', media: 'view', text: 'view', title: 'text', description: 'text', actions: 'view' } as const;

/** Centred and quiet: no fill or border of its own, so it sits in whatever holds it. */
export const emptyStateRecipe = {
  root: {
    base: 'self-stretch items-center-safe text-center',
    size: { sm: 'gap-3 px-4 py-6', md: 'gap-4 px-6 py-12' } satisfies Record<EmptyStateSize, string>,
  },
  media: {
    base: 'items-center justify-center text-faint',
    size: { sm: 'size-10', md: 'size-14' } satisfies Record<EmptyStateSize, string>,
  },
  text: {
    /** A measure that keeps the description to a few short lines. */
    base: 'items-center gap-1 max-w-80',
  },
  title: {
    base: 'font-semibold text-text',
    size: { sm: 'text-body', md: 'text-callout' } satisfies Record<EmptyStateSize, string>,
  },
  description: {
    base: 'text-muted',
    size: { sm: 'text-footnote', md: 'text-body' } satisfies Record<EmptyStateSize, string>,
  },
  actions: {
    base: 'flex-row flex-wrap justify-center gap-2 pt-1',
  },
} as const;
