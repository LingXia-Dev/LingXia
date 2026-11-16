export const tabsParts = {
  root: 'view',
  frame: 'view',
  scroller: 'scroll',
  list: 'view',
  track: 'view',
  indicator: 'view',
  tab: 'button',
  label: 'text',
  panel: 'view',
} as const;

/**
 * A row of tabs over one panel: equal shares of the row, or each as wide as its label in a row
 * that scrolls. The indicator is one box a tab wide that slides to the chosen tab; reduced motion lands it at once. Keyboard focus is a fill on an underline tab, kept
 * 4 px above the indicator by the frame's padding. The segmented variant draws SegmentedControl's
 * medium pieces (track, thumb, segment, label), so the two never drift apart.
 */
export const tabsRecipe = {
  root: {
    base: 'gap-4',
  },
  frame: {
    base: '',
    underline: 'pb-1 border-b border-text/8',
    // The row scrolls inside the rule, so the rule spans the tabs' area, not their content.
    fitted: 'border-b border-text/8',
  },
  scroller: {
    base: 'overflow-x-auto',
  },
  /** Inside the frame's padding, so a tab's width is exactly a share of the list. */
  list: {
    base: 'grow relative flex-row',
    // The scroll clips, so the indicator sits inside the row's own foot.
    fitted: 'relative flex-row self-start pb-1',
  },
  track: {
    base: 'absolute inset-0 rtl:-scale-x-100',
    inner: 'absolute inset-0',
  },
  indicator: {
    base: 'absolute start-0 transition-transform duration-base ease-spring',
    underline: '-bottom-[5px] h-0.5 rounded-full bg-accent',
    fitted: 'bottom-0 h-0.5 rounded-full bg-accent',
  },
  tab: {
    base: 'grow shrink basis-0 min-w-0 flex-row items-center-safe justify-center transition-colors duration-fast ease-standard disabled:opacity-40',
    underline: 'h-control-md px-2 rounded-control hover:bg-text/4 focus-visible:bg-text/7',
    fitted: 'shrink-0 flex-row items-center justify-center h-control-md px-3 rounded-control transition-colors duration-fast ease-standard hover:bg-text/4 focus-visible:bg-text/7 disabled:opacity-40',
  },
  /** A label too long for its share of the row is cut short rather than spilling into the next tab. */
  label: {
    base: 'font-medium',
    cut: 'truncate',
    whole: 'whitespace-nowrap',
    underline: 'text-footnote',
    selected: 'text-text',
    unselected: 'text-muted',
  },
  panel: {
    base: '',
  },
} as const;
