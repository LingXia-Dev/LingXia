import type { ProgressBarTone } from '../ProgressBar/ProgressBar';
import type { CircularProgressSize } from './CircularProgress';

export const circularProgressParts = {
  root: 'view',
  ring: 'view',
  track: 'vector',
  fill: 'vector',
  value: 'text',
} as const;

/** A ring: the track in the line role, as ProgressBar's, the arc in the tone, the app's words in the middle. */
export const circularProgressRecipe = {
  /** The ring, and the words beside it when they outgrow its middle. */
  root: {
    base: 'shrink-0 flex-row items-center gap-2',
  },
  ring: {
    base: 'relative shrink-0 items-center justify-center',
    size: { sm: 'size-6', md: 'size-12', lg: 'size-20' } satisfies Record<CircularProgressSize, string>,
  },
  track: {
    base: 'absolute inset-0 size-full text-line',
  },
  fill: {
    base: 'absolute inset-0 size-full',
    tone: { accent: 'text-accent', success: 'text-success-mark', warning: 'text-warning-mark', danger: 'text-danger-mark' } satisfies Record<ProgressBarTone, string>,
    indeterminate: 'motion-safe:animate-spin',
  },
  value: {
    base: 'font-semibold text-text tabular-nums',
    size: { sm: '', md: 'text-caption', lg: 'text-callout' } satisfies Record<CircularProgressSize, string>,
    /** An unseen copy on one line, measured against the ring's middle. */
    ghost: 'absolute text-transparent whitespace-nowrap',
  },
} as const;
