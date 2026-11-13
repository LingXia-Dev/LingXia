import type { VideoPlayerVariant } from './VideoPlayer';

export const videoPlayerParts = {
  root: 'view',
  frame: 'view',
  video: 'video',
  controls: 'view',
} as const;

/** The picture fills its frame; the video backs itself in black where the picture leaves room. */
export const videoPlayerRecipe = {
  root: {
    variant: { overlay: 'relative overflow-hidden rounded-xl', bar: 'gap-1' } satisfies Record<VideoPlayerVariant, string>,
  },
  frame: {
    base: 'overflow-hidden rounded-xl',
  },
  video: {
    base: 'w-full',
  },
} as const;
