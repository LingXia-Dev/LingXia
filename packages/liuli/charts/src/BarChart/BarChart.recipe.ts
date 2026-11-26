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
    ruled: 'border-t border-line',
    floored: 'border-b border-line',
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
    base: 'h-px shrink-0 bg-line-strong -translate-y-full',
  },
  slots: {
    base: 'absolute inset-0 flex-row items-stretch',
  },
  slot: {
    base: 'grow basis-0 min-w-0 items-center rounded-[calc(var(--radius-sm)*0.66667)]',
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
  /** Horizontal: a line per category, its name and figure over a bar that runs from the start. */
  lying: {
    lines: 'gap-1 -mx-1.5',
    line: 'items-stretch gap-1.5 px-1.5 py-1.5 rounded-[calc(var(--radius-sm)*0.66667)]',
    pressable: 'hover:bg-text/4 focus-visible:bg-text/7',
    head: 'flex-row items-baseline gap-3',
    name: 'grow basis-0 min-w-0 truncate text-start text-footnote text-text',
    chosen: 'font-medium',
    figure: 'shrink-0 text-footnote text-muted tabular-nums',
    track: 'relative self-stretch',
    // Drawn from the left; in a right-to-left page the bars run from the right.
    drawing: 'absolute inset-0 motion-safe:transition-[d] rtl:-scale-x-100',
    zero: 'absolute top-0 bottom-0 w-px -ms-px bg-line-strong',
    /** A scale's hairline through the track, under the bars. */
    grid: 'absolute top-0 bottom-0 w-px -ms-px bg-line',
    /** A reference across the bar, a little past it each side. */
    reference: 'absolute -top-1 -bottom-1 w-0.5 -ms-px rounded-full bg-muted',
    scale: 'relative h-4 mx-1.5 -mt-0.5',
    tick: 'absolute top-0 text-caption text-muted tabular-nums',
    key: 'flex-row flex-wrap gap-x-4 gap-y-1 pt-1',
    keyItem: 'flex-row items-center gap-1.5',
    keyMark: 'w-0.5 h-3 rounded-full bg-muted',
    keyText: 'text-caption text-muted tabular-nums',
  },
  label: {
    base: 'text-caption text-muted tabular-nums',
    cell: 'grow basis-0 min-w-0 truncate text-center',
  },
} as const;
