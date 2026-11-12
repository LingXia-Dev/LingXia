import type { SliderSize } from './Slider';

export const sliderParts = {
  root: 'view',
  header: 'view',
  label: 'text',
  value: 'text',
  control: 'slider',
  description: 'text',
} as const;

/**
 * Label row, slider, help text. The primitive draws its own track and thumb 12 in from its edges;
 * the control's negative margin lines the track up with the label above it.
 */
export const sliderRecipe = {
  root: {
    base: 'gap-1',
  },
  header: {
    base: 'flex-row items-baseline justify-between gap-3',
  },
  label: {
    base: 'font-medium text-text',
    size: { sm: 'text-caption', md: 'text-footnote' } satisfies Record<SliderSize, string>,
    disabled: 'text-faint',
    hidden: 'sr-only',
  },
  value: {
    base: 'tabular-nums text-muted',
    size: { sm: 'text-caption', md: 'text-footnote' } satisfies Record<SliderSize, string>,
  },
  control: {
    base: '-mx-3 p-0 text-accent',
    size: { sm: 'h-control-sm', md: 'h-control-md' } satisfies Record<SliderSize, string>,
  },
  description: {
    base: 'text-footnote text-muted',
  },
} as const;
