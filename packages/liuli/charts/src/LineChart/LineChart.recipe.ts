export const lineChartParts = {
  frame: 'view',
  scale: 'view',
  scaleLabel: 'text',
  plot: 'view',
  scrubber: 'view',
  rule: 'view',
  drawing: 'vector',
  layer: 'view',
  reading: 'view',
  end: 'view',
  endLabel: 'text',
  labels: 'view',
  label: 'text',
} as const;

/**
 * The drawing stretches to the plot and keeps its stroke widths. Its data sits half a marker
 * inside the plot, and the scale and end figure share that inset, so each label sits level with
 * what it names. The scale is a hairline on each round number, firmer under the values.
 */
export const lineChartRecipe = {
  frame: {
    base: 'grow flex-row items-stretch',
  },
  inset: {
    base: 'pt-1',
    open: 'pb-1',
  },
  scale: {
    base: 'shrink-0 items-end pe-2',
  },
  scaleLabel: {
    base: 'text-footnote text-muted tabular-nums whitespace-nowrap',
    // A box taller than its line is not centred on it by layout; half its own height up is.
    level: '-translate-y-1/2',
  },
  plot: {
    base: 'relative grow basis-0 min-w-0',
  },
  scrubber: {
    base: 'absolute inset-0 rounded-[calc(var(--radius-sm)*0.66667)]',
  },
  // The hairlines of the scale, laid over the plot in the same inset as the drawing.
  grid: {
    base: 'absolute inset-0',
  },
  /** Holds the movers: the plot's height, out of which they take their share. */
  track: {
    base: 'grow self-stretch relative',
  },
  /** A box as tall as the plot, moved down by a value's share of it on the lines' timing. */
  mover: {
    base: 'motion-safe:transition-transform duration-slow ease-standard',
    tick: 'absolute inset-0',
    end: 'grow',
  },
  // A tick's line at the top of its mover: no height of its own.
  tick: {
    base: 'h-0 self-stretch items-end',
  },
  rule: {
    line: 'self-stretch h-px bg-line',
    foot: 'self-stretch h-px bg-line-strong',
    bottom: 'absolute start-0 end-0 bottom-0 h-px bg-line-strong',
    open: 'bottom-1',
  },
  drawing: {
    base: 'w-full grow object-fill',
    /** The lines draw in once laid out and move to new data; so does the marker, which a scrub replaces instead. */
    moves: 'motion-safe:transition-[d] duration-slow ease-standard',
    over: 'absolute inset-0 h-full',
  },
  /** Fills under the lines and dots over them: they fade in once a line has drawn in. */
  layer: {
    base: 'absolute inset-0 motion-safe:transition-opacity duration-base delay-300 ease-standard',
    waiting: 'opacity-0',
  },
  reading: {
    base: 'sr-only',
  },
  end: {
    base: 'shrink-0 ps-1.5',
    arrives: 'motion-safe:transition-opacity duration-base delay-300 ease-standard',
  },
  endAnchor: {
    base: 'h-0',
  },
  endLabel: {
    base: 'text-footnote font-medium text-text tabular-nums whitespace-nowrap',
    // A box taller than its line is not centred on it by layout; half its own height up is.
    level: '-translate-y-1/2',
  },
  ghost: {
    base: 'h-0 overflow-hidden',
  },
  labels: {
    base: 'flex-row pt-3',
    ends: 'grow basis-0 flex-row justify-between',
    cover: 'absolute inset-0 flex-row justify-between',
  },
  label: {
    base: 'text-caption text-muted tabular-nums',
  },
} as const;
