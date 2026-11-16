import type { ThumbnailRatio, ThumbnailSize } from './Thumbnail';

export const thumbnailParts = {
  root: 'view',
  image: 'image',
} as const;

/** A small picture on a faint tint that shows until it loads; as a button it takes the focus ring. */
export const thumbnailRecipe = {
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
} as const;
