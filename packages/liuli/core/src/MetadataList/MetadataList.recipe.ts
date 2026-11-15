import type { MetadataListLayout } from './MetadataList';

export const metadataListParts = {
  root: 'view',
  item: 'view',
  label: 'text',
  value: 'text',
  content: 'view',
} as const;

/**
 * Labels are quiet and values carry the weight. In rows the labels share one column, so the
 * values line up; stacked, each label sits over its value in a grid.
 */
export const metadataListRecipe = {
  root: {
    layout: { rows: 'self-stretch gap-2.5', stacked: 'self-stretch grid gap-x-4 gap-y-3' } satisfies Record<MetadataListLayout, string>,
    columns: { 1: 'grid-cols-1', 2: 'grid-cols-2', 3: 'grid-cols-3' } satisfies Record<1 | 2 | 3, string>,
  },
  item: {
    layout: { rows: 'flex-row items-start gap-3', stacked: 'gap-0.5 min-w-0' } satisfies Record<MetadataListLayout, string>,
  },
  label: {
    base: 'text-footnote text-muted',
    // A third of the row, so the values start on one line down the list.
    layout: { rows: 'w-1/3 shrink-0 pt-0.5', stacked: '' } satisfies Record<MetadataListLayout, string>,
  },
  value: {
    base: 'text-body text-text',
    layout: { rows: 'grow shrink basis-0 min-w-0', stacked: '' } satisfies Record<MetadataListLayout, string>,
  },
  content: {
    layout: { rows: 'grow shrink basis-0 min-w-0 flex-row items-center', stacked: 'flex-row items-center' } satisfies Record<MetadataListLayout, string>,
  },
} as const;
