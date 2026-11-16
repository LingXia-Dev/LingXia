import type { Layer } from '../internal/layer';

export const detailsParts = {
  group: 'view',
  root: 'view',
  summary: 'button',
  icon: 'vector',
  text: 'view',
  label: 'text',
  description: 'text',
  chevron: 'vector',
  box: 'view',
  body: 'view',
} as const;

/** A disclosure row: the summary is the press target, the chevron turns as it opens. */
export const detailsRecipe = {
  group: {
    base: 'overflow-hidden rounded-container border border-text/12 divide-y divide-text/8',
    /** One rung up from what it sits on; inside an overlay it takes the overlay's surface. */
    layer: { page: 'bg-surface', overlay: 'bg-surface-overlay' } satisfies Record<Layer, string>,
  },
  /** A row moves with the one above it as that one opens or closes. */
  root: {
    base: 'transition-layout duration-base ease-standard',
  },
  summary: {
    base: 'flex-row items-center gap-3 min-h-control-lg py-2.5 text-start transition-colors duration-fast ease-standard hover:bg-text/4 active:bg-text/7 focus-visible:bg-text/7 disabled:opacity-40',
    /** Alone, the hover wash reaches past the text, which stays in line with the content around it. */
    alone: '-mx-3 px-3 rounded-control',
    inGroup: 'px-4',
  },
  icon: {
    base: 'size-5 shrink-0 text-muted',
  },
  text: {
    base: 'grow shrink basis-0 min-w-0 gap-0.5',
  },
  label: {
    base: 'text-body font-medium text-text',
  },
  description: {
    base: 'text-footnote text-muted',
  },
  chevron: {
    base: 'size-5 shrink-0 text-muted transition-transform duration-base ease-standard',
    open: '-rotate-180',
  },
  /** The body's clip: it grows from nothing to the body's height and back; under reduced motion it changes at once. */
  box: {
    base: 'overflow-hidden transition-[height] duration-base ease-standard motion-reduce:transition-none',
    closed: 'h-0',
  },
  body: {
    base: 'pb-4 gap-2',
    inGroup: 'px-4',
  },
} as const;
