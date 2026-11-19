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
 * pieces are the segmented Tabs' too. A label never shrinks below its width: a long one widens its
 * segment, and a row that cannot hold them all stacks. Reduced motion lands the thumb at once.
 */
export const segmentedControlRecipe = {
  root: {
    // Dark `surface-sunken` sits a hair under the canvas; an inset hairline draws the track's edge on any surface.
    base: 'relative p-0.5 rounded-control bg-surface-sunken inset-ring inset-ring-text/8',
    row: 'flex-row',
    stacked: 'flex-col',
  },
  /** Inside the padding, so a segment is exactly a share of the row; mirrors in right-to-left. */
  track: {
    base: 'absolute inset-0.5 rtl:-scale-x-100',
    inner: 'absolute inset-0',
  },
  thumb: {
    /** The hairline carries the lift in dark, where the contact shadow does not show. */
    base: 'absolute start-0 rounded-segment border border-text/12 bg-surface-raised shadow-contact transition-transform duration-base ease-spring',
    row: 'inset-y-0',
    stacked: 'top-0 end-0',
  },
  segment: {
    base: 'flex-row items-center-safe justify-center px-1 rounded-segment transition-colors duration-fast ease-standard focus-visible:ring-2 focus-visible:ring-focus focus-visible:ring-offset-2 disabled:opacity-40',
    /** With the root's padding, the row is control-sm or control-md tall, level with a Button or field of that size. */
    size: { sm: 'h-7', md: 'h-9' } satisfies Record<SegmentedControlSize, string>,
    /** Equal shares, but never narrower than the label. */
    row: 'grow basis-0',
    stacked: '',
    /** Fills its segment, so the thumb can follow segments of unequal width. */
    extent: 'absolute inset-0',
  },
  /** Whole in a row; cut short only when a stacked segment, the full width, cannot hold it. */
  label: {
    base: 'font-medium',
    row: 'whitespace-nowrap',
    stacked: 'truncate',
    size: { sm: 'text-footnote', md: 'text-body' } satisfies Record<SegmentedControlSize, string>,
    chosen: 'text-text',
    other: 'text-muted',
  },
} as const;
