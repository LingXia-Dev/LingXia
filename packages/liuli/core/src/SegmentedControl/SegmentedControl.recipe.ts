import type { SegmentedControlSize } from './SegmentedControl';

export const segmentedControlParts = {
  root: 'view',
  track: 'view',
  thumb: 'view',
  segment: 'button',
  label: 'text',
} as const;

/**
 * A sunken track of equal segments with one raised thumb that slides to the chosen segment; the
 * pieces are the segmented Tabs' too. Reduced motion lands the thumb at once.
 */
export const segmentedControlRecipe = {
  root: {
    base: 'relative flex-row p-0.5 rounded-control bg-surface-sunken',
  },
  /** Inside the padding, so a segment is exactly a share of the row; mirrors in right-to-left. */
  track: {
    base: 'absolute inset-0.5 rtl:-scale-x-100',
    inner: 'absolute inset-0',
  },
  thumb: {
    /** The hairline carries the lift in dark, where the contact shadow does not show. */
    base: 'absolute start-0 inset-y-0 rounded-segment border border-text/12 bg-surface-raised shadow-contact transition-transform duration-200 ease-spring',
  },
  segment: {
    base: 'grow shrink basis-0 min-w-0 flex-row items-center justify-center px-1 rounded-segment transition-colors duration-150 ease-out focus-visible:ring-2 focus-visible:ring-focus focus-visible:ring-offset-2 disabled:opacity-40',
    /** With the root's padding, the row is control-sm or control-md tall, level with a Button or field of that size. */
    size: { sm: 'h-7', md: 'h-9' } satisfies Record<SegmentedControlSize, string>,
  },
  /** A label too long for its share is cut short rather than spilling into the next segment. */
  label: {
    base: 'font-medium truncate',
    size: { sm: 'text-footnote', md: 'text-body' } satisfies Record<SegmentedControlSize, string>,
    chosen: 'text-text',
    other: 'text-muted',
  },
} as const;
