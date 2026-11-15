export const carouselParts = {
  root: 'view',
  scroll: 'scroll',
  track: 'view',
  page: 'view',
  dots: 'view',
} as const;

/** Pages side by side in a scroll that stops on each; the dots sit under it, or over its bottom edge on media. */
export const carouselRecipe = {
  root: {
    base: 'relative self-stretch gap-1',
  },
  scroll: {
    base: 'self-stretch overflow-x-auto snap-x snap-always rounded-container',
  },
  track: {
    base: 'flex-row',
  },
  page: {
    base: 'w-full shrink-0 snap-start',
  },
  dots: {
    over: 'absolute inset-x-0 bottom-1',
  },
} as const;
