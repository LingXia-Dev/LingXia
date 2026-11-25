import type { ProgressBarSize, ProgressBarTone, SpinnerSize, SpinnerTone } from './ProgressBar';

export const progressBarParts = {
  root: 'view',
  header: 'view',
  label: 'text',
  value: 'text',
  track: 'view',
  fill: 'view',
  dashes: 'view',
  spinner: 'vector',
  loading: 'view',
} as const;

/** ProgressBar and Spinner classes by part. The fill's width is the one value set inline. */
export const progressBarRecipe = {
  root: {
    base: 'gap-1.5',
  },
  header: {
    base: 'flex-row items-baseline justify-between gap-3',
  },
  label: {
    base: 'grow basis-0 text-footnote font-medium text-text',
  },
  value: {
    base: 'text-footnote text-muted tabular-nums',
  },
  track: {
    base: 'self-stretch overflow-hidden rounded-full bg-line',
    size: { sm: 'h-1', md: 'h-2' } satisfies Record<ProgressBarSize, string>,
  },
  fill: {
    base: 'h-full rounded-full',
    tone: { accent: 'bg-accent', success: 'bg-success-mark', warning: 'bg-warning-mark', danger: 'bg-danger-mark' } satisfies Record<ProgressBarTone, string>,
    determinate: 'transition-layout duration-base ease-standard',
  },
  /**
   * An unknown amount: dashes with half a dash between, a pattern no value draws, starting and ending
   * on a whole dash. With motion, a box one period wide marches the row in the reading direction.
   */
  dashes: {
    row: 'h-full flex-row',
    march: { ltr: 'h-full motion-safe:animate-progress-march', rtl: 'h-full motion-safe:animate-progress-march-rtl' },
    dash: 'h-full shrink-0 rounded-full',
  },
  spinner: {
    base: 'motion-safe:animate-spin',
    tone: { accent: 'text-accent', muted: 'text-muted', 'on-media': 'text-on-media' } satisfies Record<SpinnerTone, string>,
    size: { sm: 'size-4', md: 'size-6', lg: 'size-8', xl: 'size-12' } satisfies Record<SpinnerSize, string>,
  },
  /** A Spinner that shows its label: the ring over the words, centred. */
  loading: {
    base: 'items-center gap-2',
    label: 'text-footnote text-center',
    tone: { accent: 'text-muted', muted: 'text-muted', 'on-media': 'text-on-media' } satisfies Record<SpinnerTone, string>,
  },
} as const;
