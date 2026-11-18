export const dateRangeInputParts = {
  value: 'text',
  popover: 'view',
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
  popover: {
    base: 'mt-1 w-[312px] max-w-[90vw] p-3 rounded-container border border-text/12 bg-surface-overlay shadow-raised transition duration-fast ease-standard starting:opacity-0 exit:opacity-0 motion-safe:starting:-translate-y-1',
  },
} as const;
