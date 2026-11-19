export const barChartParts = {
  scale: 'text',
  plot: 'view',
  marks: 'view',
  region: 'view',
  wrap: 'view',
  drawing: 'vector',
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
 * handful of elements however many bars it has. The drawing stretches to its region; a stroke of
 * constant width rounds each bar's corners, and each region hides the stroke at the zero line.
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
    // With figures, the figures' line is kept clear above the tallest bar, and below the deepest.
    base: 'absolute inset-0',
  },
  region: {
    base: 'relative basis-0 overflow-hidden',
  },
  wrap: {
    // Each drawing is taller than its region by the corner radius, on the zero line's side, so a
    // bar's height is exact and the stroke there is hidden.
    base: 'absolute start-0 end-0',
    up: { regular: 'top-0 -bottom-[3px] pt-[3px]', dense: 'top-0 -bottom-px pt-px' },
    down: { regular: 'bottom-0 -top-[3px] pb-[3px]', dense: 'bottom-0 -top-px pb-px' },
  },
  drawing: {
    base: 'w-full grow object-fill',
    full: 'opacity-85',
    dimmed: 'opacity-30',
  },
  baseline: {
    base: 'h-px shrink-0 bg-text/20',
  },
  slots: {
    base: 'absolute inset-0 flex-row items-stretch',
  },
  slot: {
    base: 'grow basis-0 min-w-0 items-center rounded-[4px]',
    pressable: 'hover:bg-text/4 focus-visible:bg-text/7',
  },
  space: {
    base: 'basis-0',
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
