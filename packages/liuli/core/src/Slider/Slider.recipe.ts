import type { SliderVariant } from './Slider';

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
    base: '-mx-3 p-0 h-control-md text-accent',
  },
  description: {
    base: 'text-footnote text-muted',
  },
} as const;
