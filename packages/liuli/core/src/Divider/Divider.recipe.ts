import type { DividerInset } from './Divider';

export const dividerParts = {
  line: 'view',
  labelled: 'view',
  label: 'text',
} as const;

/** The subtle edge: the same hairline that parts rows in a list. */
export const dividerRecipe = {
  line: {
    horizontal: 'self-stretch h-px shrink-0 bg-text/8',
    vertical: 'self-stretch w-px shrink-0 bg-text/8',
    side: 'grow basis-0 h-px bg-text/8',
  },
  inset: { none: '', start: 'ms-4', both: 'mx-4' } satisfies Record<DividerInset, string>,
  labelled: {
    base: 'self-stretch flex-row items-center gap-3',
  },
  label: {
    base: 'text-footnote text-muted',
  },
} as const;
