import type { ThumbnailRatio, ThumbnailSize } from './Thumbnail';

export const thumbnailParts = {
  frame: 'view',
  root: 'view',
  image: 'image',
  loading: 'view',
  remove: 'button',
} as const;

/** A small picture on a faint tint that shows until it loads; as a button it takes the focus ring. */
export const thumbnailRecipe = {
  /** Holds the picture and its remove button side by side, the button over the top corner. */
  frame: {
    base: 'relative shrink-0',
    size: { sm: 'w-12', md: 'w-18', lg: 'w-24', fill: 'self-stretch' } satisfies Record<ThumbnailSize, string>,
  },
  root: {
    base: 'relative shrink-0 overflow-hidden rounded-control bg-text/6',
    size: { sm: 'w-12', md: 'w-18', lg: 'w-24', fill: 'self-stretch' } satisfies Record<ThumbnailSize, string>,
    ratio: { square: 'aspect-square', landscape: 'aspect-4/3', wide: 'aspect-video' } satisfies Record<ThumbnailRatio, string>,
    pressable: 'transition-opacity duration-fast ease-standard active:opacity-80 focus-visible:ring-2 focus-visible:ring-focus focus-visible:ring-offset-2',
    selected: 'ring-2 ring-accent ring-offset-2',
  },
  image: {
    base: 'absolute inset-0 size-full object-cover',
  },
  /** Dims the picture under a spinner while it is still on its way. */
  loading: {
    base: 'absolute inset-0 items-center justify-center bg-media/50',
    ring: '',
  },
  /** A 32 px target over the top corner, around a small disc in the media colours, legible on any picture. */
  remove: {
    base: 'absolute -top-2 -end-2 size-8 items-center justify-center rounded-full active:opacity-80 focus-visible:ring-2 focus-visible:ring-focus',
    disc: 'size-5 items-center justify-center rounded-full border border-on-media/20 bg-media text-on-media',
    icon: 'size-3',
  },
} as const;
