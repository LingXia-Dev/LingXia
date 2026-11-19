export const lightboxParts = {
  panel: 'dialog',
  bar: 'view',
  count: 'text',
  close: 'button',
  closeIcon: 'vector',
  scroll: 'scroll',
  track: 'view',
  page: 'view',
  image: 'image',
  caption: 'text',
} as const;

/** The whole screen on the opaque media canvas: pictures shown whole, one at a time, with their controls in the on-media colour. */
export const lightboxRecipe = {
  panel: {
    base: 'w-full h-full rounded-none bg-media-canvas px-0 pt-1 pb-3 gap-2 transition duration-base ease-standard starting:opacity-0 exit:opacity-0',
  },
  bar: {
    base: 'flex-row items-center justify-between min-h-11 ps-4 pe-2',
  },
  count: {
    base: 'text-footnote font-medium text-on-media tabular-nums',
  },
  close: {
    base: 'size-11 items-center justify-center rounded-full text-on-media hover:bg-on-media/10 active:bg-on-media/20 focus-visible:ring-2 focus-visible:ring-on-media',
  },
  closeIcon: {
    base: 'size-6',
  },
  scroll: {
    base: 'grow basis-0 min-h-0 self-stretch overflow-x-auto snap-x snap-always',
  },
  track: {
    base: 'flex-row h-full',
  },
  page: {
    base: 'w-full h-full shrink-0 snap-start items-center justify-center',
  },
  /** Shown whole, never cropped. */
  image: {
    base: 'size-full object-contain',
  },
  caption: {
    base: 'px-5 text-center text-footnote text-on-media',
  },
} as const;
