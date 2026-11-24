export const barChartParts = {
  scale: 'text',
  plot: 'view',
  marks: 'view',
  drawing: 'vector',
  zero: 'view',
  baseline: 'view',
  slots: 'view',
  slot: 'button',
  valueSlot: 'view',
  value: 'text',
  labels: 'view',
  label: 'text',
} as const;

/**
 * Bars are geometry: every bar of a series is one outline in one drawing, so a chart costs a
 * handful of elements however many bars it has. The drawing stretches to the marks; a stroke of
 * constant width rounds each bar's free end and stops flat at the zero line.
 * Over the drawing lies one transparent slot per category: its press target and its spoken name.
 */
export const barChartRecipe = {
  scale: {
    base: 'pb-1 text-footnote text-muted tabular-nums',
  },
  plot: {
    base: 'relative grow',
    ruled: 'border-t border-text/8',
    floored: 'border-b border-text/8',
  },
  marks: {
    // Inset by the corner radius at the free ends, and with figures by the figures' line too.
    base: 'absolute inset-0',
  },
  /** The one timing the bars, the zero line and the figures move on, so they arrive together. */
  motion: {
    base: 'duration-slow ease-standard',
  },
  drawing: {
    base: 'absolute inset-0 object-fill motion-safe:transition-[d]',
    full: 'opacity-85',
    dimmed: 'opacity-30',
  },
  /** A box as tall as the marks, moved down to the zero line. */
  zero: {
    base: 'absolute inset-0 motion-safe:transition-transform',
  },
  /** Lies over the bars' last point at zero. */
  baseline: {
    base: 'h-px shrink-0 bg-text/20 -translate-y-full',
  },
  slots: {
    base: 'absolute inset-0 flex-row items-stretch',
  },
  slot: {
    base: 'grow basis-0 min-w-0 items-center rounded-[4px]',
    pressable: 'hover:bg-text/4 focus-visible:bg-text/7',
  },
  /** A box as tall as the plot less the figure's line, moved down to the bar's free end on the bars' timing. */
  mover: {
    base: 'grow self-stretch items-center motion-safe:transition-transform',
  },
  /** The figure's line, kept clear at the foot so the mover is the plot's height. */
  lane: {
    base: 'shrink-0',
  },
  valueSlot: {
    base: 'shrink-0 items-center justify-center-safe',
  },
  value: {
    base: 'text-footnote text-muted tabular-nums whitespace-nowrap',
    // The longest figure, laid out unseen to tell whether the figures fit side by side.
    ghost: 'absolute top-0 start-0 opacity-0',
  },
  labels: {
    row: 'pt-3',
    fit: 'flex-row pt-3',
    // The labels drawn until the row is measured, over the same line.
    cover: 'absolute inset-0',
    all: 'flex-row',
    ends: 'flex-row justify-between',
  },
  label: {
    base: 'text-caption text-muted tabular-nums',
    cell: 'grow basis-0 min-w-0 truncate text-center',
  },
} as const;
