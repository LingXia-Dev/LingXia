import type { ProgressBarSize, ProgressBarTone, SpinnerSize, SpinnerTone } from './ProgressBar';

export const progressBarParts = {
  root: 'view',
  header: 'view',
  label: 'text',
  value: 'text',
  track: 'view',
  fill: 'view',
  spinner: 'vector',
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
    base: 'self-stretch overflow-hidden rounded-full bg-text/10',
    size: { sm: 'h-1', md: 'h-2' } satisfies Record<ProgressBarSize, string>,
  },
  fill: {
    base: 'h-full rounded-full',
    tone: { accent: 'bg-accent', success: 'bg-success-mark', warning: 'bg-warning-mark', danger: 'bg-danger-mark' } satisfies Record<ProgressBarTone, string>,
    determinate: 'transition-layout duration-base ease-standard',
    indeterminate: 'w-2/5 motion-safe:animate-progress-sweep',
  },
  spinner: {
    base: 'motion-safe:animate-spin',
    tone: { accent: 'text-accent', muted: 'text-muted', 'on-media': 'text-on-media' } satisfies Record<SpinnerTone, string>,
    size: { sm: 'size-4', md: 'size-6', lg: 'size-8', xl: 'size-12' } satisfies Record<SpinnerSize, string>,
  },
} as const;
