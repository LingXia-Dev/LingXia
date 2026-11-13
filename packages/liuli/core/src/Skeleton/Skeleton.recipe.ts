import type { SkeletonShape } from './Skeleton';

export const skeletonParts = { group: 'view', lines: 'view', bone: 'view', glint: 'view' } as const;

/**
 * A faint fill in the text colour, so it reads on any surface in either theme. A soft band of the
 * surface colour glides across it; under reduced motion the band stays out of view and the fill is still.
 */
export const skeletonRecipe = {
  group: {
    base: 'gap-3',
  },
  lines: {
    base: 'self-stretch gap-2',
  },
  bone: {
    base: 'relative overflow-hidden bg-text/7',
    shape: {
      /** The body text's x-height band, centred in its line. */
      text: 'h-3 my-[5px] rounded-full',
      block: 'rounded-control',
      circle: 'rounded-full',
    } satisfies Record<SkeletonShape, string>,
    /** A paragraph's last line. */
    last: 'self-start w-3/5',
  },
  glint: {
    base: 'absolute inset-0 -translate-x-full bg-linear-90 from-transparent via-surface/50 to-transparent motion-safe:animate-skeleton-glint',
  },
} as const;
