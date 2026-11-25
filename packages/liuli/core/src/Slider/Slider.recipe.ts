import type { SliderSize, SliderVariant } from './Slider';

export const sliderParts = {
  root: 'view',
  header: 'view',
  label: 'text',
  value: 'text',
  control: 'slider',
  marks: 'view',
  mark: 'view',
  tick: 'view',
  markLabel: 'text',
  description: 'text',
} as const;

/**
 * Label row, slider, help text. The thumb is a white disc with an accent rim, so it reads on a
 * dark page, on the range and on the plain track alike, and grows a little under a finger. The
 * range is the accent at 80 %, deeper in dark, so the knob keeps 3:1 against it. The primitive sets
 * the track ends in by half the thumb; the control's negative margin lines them up with the label.
 */
export const sliderRecipe = {
  root: {
    base: 'gap-1',
    variant: { standalone: '', row: 'self-stretch px-4 pt-2.5 pb-1.5' } satisfies Record<SliderVariant, string>,
  },
  header: {
    base: 'flex-row items-baseline justify-between gap-3',
  },
  label: {
    base: 'text-text',
    variant: { standalone: 'text-footnote font-medium', row: 'text-body' } satisfies Record<SliderVariant, string>,
    disabled: 'text-faint',
    hidden: 'sr-only',
  },
  value: {
    base: 'tabular-nums text-muted',
    variant: { standalone: 'text-footnote', row: 'text-body' } satisfies Record<SliderVariant, string>,
  },
  control: {
    base: 'p-0 text-accent track:rounded-full track:bg-line range:rounded-full range:bg-accent/80 secondary:rounded-full secondary:bg-line-strong/50 thumb:rounded-full thumb:bg-knob thumb:border-2 thumb:border-accent thumb:shadow-contact hover:thumb:scale-105 active:thumb:scale-110 focus-visible:thumb:ring-2 focus-visible:thumb:ring-focus focus-visible:thumb:ring-offset-2 disabled:range:bg-accent/40 disabled:thumb:bg-surface disabled:thumb:border-accent/40 disabled:thumb:shadow-none',
    size: {
      sm: '-mx-2 h-control-sm track:h-[3px] range:h-[3px] secondary:h-[3px] thumb:size-4',
      md: '-mx-2.5 h-control-md track:h-1 range:h-1 secondary:h-1 thumb:size-5',
    } satisfies Record<SliderSize, string>,
  },
  /** Ticks under the track, each at its value's share of the width; the thumb's centre travels the same span. */
  marks: {
    base: 'flex-row items-start',
    /** Drawn up under the track, into the control's own height. */
    size: { sm: '-mt-2', md: '-mt-2.5' } satisfies Record<SliderSize, string>,
  },
  /** A zero-width seat, so the tick sits on the value and the label spreads to both sides; at the ends the label keeps inside. */
  mark: {
    base: 'w-0 gap-1',
    edge: { start: 'items-start', middle: 'items-center', end: 'items-end' },
  },
  tick: {
    base: 'w-px h-1.5 bg-line-strong',
  },
  markLabel: {
    base: 'text-caption text-muted whitespace-nowrap tabular-nums',
  },
  description: {
    base: 'text-footnote text-muted whitespace-pre-line',
  },
} as const;
