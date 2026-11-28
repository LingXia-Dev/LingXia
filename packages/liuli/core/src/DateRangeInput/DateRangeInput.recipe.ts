export const dateRangeInputParts = {
  value: 'text',
  icon: 'vector',
  popover: 'view',
  presets: 'view',
  preset: 'button',
  months: 'view',
  month: 'view',
} as const;

/**
 * The field is Combobox's box with a calendar mark, and on touch its sheet; the dates are in
 * tabular figures so a span keeps its width as its days change. With a mouse the calendar hangs
 * under the field in a panel as wide as seven days need.
 */
export const dateRangeInputRecipe = {
  value: {
    base: 'tabular-nums',
  },
  // DateInput's calendar, so the two date fields match.
  icon: {
    base: 'size-4 shrink-0 text-muted',
  },
  /** Named spans over the calendar, wrapping; quiet pills that turn accent when the field holds their span. */
  presets: {
    base: 'flex-row flex-wrap gap-2 pb-3 mb-3 border-b border-line',
  },
  preset: {
    base: 'min-h-8 px-3 items-center justify-center rounded-full border border-line-strong bg-transparent hover:bg-text/5 active:bg-text/8 focus-visible:ring-2 focus-visible:ring-focus focus-visible:ring-offset-2 transition duration-fast ease-standard',
    chosen: 'border-accent bg-accent-soft hover:bg-accent-soft',
    label: 'text-footnote font-medium text-text',
  },
  popover: {
    base: 'mt-1 max-w-[90vw] p-3 rounded-container border border-line bg-surface-overlay shadow-raised transition duration-fast ease-standard starting:opacity-0 exit:opacity-0 motion-safe:starting:-translate-y-1',
    single: 'w-78',
    // Two months and the gap between them.
    pair: 'w-162',
  },
  months: {
    base: 'flex-row items-start gap-6',
  },
  month: {
    base: 'grow basis-0 min-w-0',
  },
} as const;
