export const lineChartParts = {
  frame: 'view',
  scale: 'view',
  scaleLabel: 'text',
  plot: 'view',
  scrubber: 'view',
  rule: 'view',
  drawing: 'vector',
  reading: 'view',
  end: 'view',
  endLabel: 'text',
  labels: 'view',
  label: 'text',
} as const;

/**
 * The drawing stretches to the plot and keeps its stroke widths. Drawing, scale and end figure
 * share one inset of half a marker, so a dot on the first or last value is whole and each label
 * sits level with what it names. The rules are two hairlines: the top of the scale and the foot.
 */
export const lineChartRecipe = {
  frame: {
    base: 'grow flex-row items-stretch',
  },
  inset: {
    base: 'pt-[4px]',
    open: 'pb-[4px]',
  },
  scale: {
    base: 'shrink-0 items-end justify-between pe-2',
  },
  scaleLabel: {
    base: 'text-footnote text-muted tabular-nums whitespace-nowrap',
    top: '-translate-y-1/2',
    bottom: 'translate-y-1/2',
  },
  plot: {
    base: 'relative grow basis-0 min-w-0',
  },
  scrubber: {
    base: 'absolute inset-0 rounded-[4px]',
  },
  rule: {
    top: 'absolute start-0 end-0 top-[4px] h-px bg-text/8',
    bottom: 'absolute start-0 end-0 bottom-0 h-px bg-text/12',
    open: 'bottom-[4px]',
  },
  drawing: {
    base: 'w-full grow object-fill px-[4px]',
    over: 'absolute inset-0 h-full',
  },
  reading: {
    base: 'sr-only',
  },
  end: {
    base: 'shrink-0 ps-1.5',
  },
  space: {
    base: 'basis-0',
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
