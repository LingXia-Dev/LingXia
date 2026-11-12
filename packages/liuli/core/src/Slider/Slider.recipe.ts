import type { SliderSize, SliderVariant } from './Slider';

export const sliderParts = {
  root: 'view',
  header: 'view',
  label: 'text',
  value: 'text',
  control: 'slider',
  description: 'text',
} as const;

/**
 * Label row, slider, help text. The thumb is a raised disc with a hairline, so it reads on the
 * accent range and the plain track alike, and grows a little under a finger. The primitive sets the
 * track ends in by half the thumb; the control's negative margin lines them up with the label.
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
    base: 'p-0 text-accent track:rounded-full track:bg-line range:rounded-full range:bg-accent secondary:rounded-full secondary:bg-line-strong/50 thumb:rounded-full thumb:bg-surface-raised thumb:border thumb:border-text/10 thumb:shadow-contact active:thumb:scale-110 focus-visible:thumb:ring-2 focus-visible:thumb:ring-focus disabled:range:bg-accent/40 disabled:thumb:shadow-none',
    size: {
      sm: '-mx-2 h-control-sm track:h-[3px] range:h-[3px] secondary:h-[3px] thumb:size-4',
      md: '-mx-2.5 h-control-md track:h-1 range:h-1 secondary:h-1 thumb:size-5',
    } satisfies Record<SliderSize, string>,
  },
  description: {
    base: 'text-footnote text-muted',
  },
} as const;
