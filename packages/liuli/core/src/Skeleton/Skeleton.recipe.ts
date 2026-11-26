import type { SkeletonShape, SkeletonTextSize } from './Skeleton';

export const skeletonParts = { group: 'view', lines: 'view', line: 'view', ghost: 'text', size: 'view', bone: 'view', glint: 'view' } as const;

/**
 * A faint fill in the text colour, so it reads on any surface in either theme. A soft band of the
 * surface colour glides across it; under reduced motion the band stays out of view and the fill is still.
 */
export const skeletonRecipe = {
  group: {
    base: 'gap-3',
  },
  /** Lines follow one another as a paragraph's do, with no gap of their own. */
  lines: {
    base: 'self-stretch',
  },
  /** One line of the text: its height from the ghost run, the bar across it. */
  line: {
    base: 'flex-row items-center self-stretch',
    /** A paragraph's last line. */
    last: 'self-start w-3/5',
  },
  ghost: {
    base: 'w-0 shrink-0 overflow-hidden',
  },
  /** The type a text bar stands in for; the bar's height is in its em, so it grows with the text. */
  size: {
    caption: 'text-caption',
    footnote: 'text-footnote',
    body: 'text-body',
    callout: 'text-callout',
    title: 'text-title',
    display: 'text-display',
  } satisfies Record<SkeletonTextSize, string>,
  bone: {
    /** 1.22:1 on surface and canvas alike in light (7 % faded to 1.15 on white); 1.3:1 in dark. */
    base: 'relative overflow-hidden bg-text/10',
    shape: {
      /** The text's x-height band, a little over half its em. */
      text: 'grow h-[0.7em] rounded-full',
      block: 'rounded-control',
      circle: 'rounded-full',
    } satisfies Record<SkeletonShape, string>,
  },
  glint: {
    base: 'absolute inset-0 -translate-x-full bg-linear-90 from-transparent via-surface/50 to-transparent motion-safe:animate-skeleton-glint',
  },
} as const;
