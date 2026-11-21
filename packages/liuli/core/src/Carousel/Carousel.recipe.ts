export const carouselParts = {
  root: 'view',
  frame: 'view',
  scroll: 'scroll',
  track: 'view',
  page: 'view',
  button: 'view',
  dots: 'view',
} as const;

/**
 * Pages side by side in a scroll that stops on each; the dots sit under it, or over its bottom edge on media.
 * The buttons are small round chips on the overlay surface over the side edges, so they read on a picture.
 */
export const carouselRecipe = {
  root: {
    base: 'relative self-stretch gap-1',
  },
  frame: {
    base: 'relative self-stretch',
  },
  scroll: {
    base: 'self-stretch overflow-x-auto snap-x snap-always rounded-container',
  },
  track: {
    base: 'flex-row',
    rail: 'gap-3',
  },
  page: {
    base: 'shrink-0 snap-start',
    full: 'w-full',
  },
  button: {
    // The full height of the edge, centring its chip.
    base: 'absolute inset-y-0 justify-center',
    chip: 'rounded-full bg-surface-overlay shadow-raised',
    start: 'start-2',
    end: 'end-2',
  },
  dots: {
    over: 'absolute inset-x-0 bottom-1',
  },
} as const;
