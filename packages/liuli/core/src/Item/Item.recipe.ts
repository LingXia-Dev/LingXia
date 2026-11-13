import type { Layer } from '../internal/layer';
import type { ItemTone } from './Item';

export const itemParts = {
  list: 'view',
  listitem: 'view',
  root: 'view',
  start: 'view',
  text: 'view',
  label: 'text',
  description: 'text',
  end: 'view',
  chevron: 'vector',
} as const;

/** List row classes by part. A pressable row lands on a `button`, a plain one on a `view`. */
export const itemRecipe = {
  list: {
    base: 'overflow-hidden rounded-container border border-text/12 divide-y divide-text/8',
    /** One rung up from what it sits on; inside an overlay it takes the overlay's surface. */
    layer: { page: 'bg-surface', raised: 'bg-surface-raised' } satisfies Record<Layer, string>,
  },
  listitem: {
    base: 'self-stretch',
  },
  root: {
    base: 'flex-row items-center gap-3 px-4 py-2.5 min-h-control-lg self-stretch text-start',
    pressable: 'transition-colors duration-150 ease-out hover:bg-text/4 active:bg-text/7 focus-visible:bg-text/7 disabled:opacity-40',
  },
  start: {
    base: 'shrink-0 items-center justify-center text-muted',
  },
  text: {
    base: 'grow shrink basis-0 gap-0.5',
  },
  label: {
    base: 'text-body',
    tone: {
      default: 'text-text',
      danger: 'text-danger',
    } satisfies Record<ItemTone, string>,
  },
  description: {
    base: 'text-footnote text-muted',
  },
  end: {
    base: 'shrink-0 flex-row items-center gap-2 text-muted',
  },
  chevron: {
    base: 'size-5 shrink-0 text-faint rtl:-scale-x-100',
  },
} as const;
