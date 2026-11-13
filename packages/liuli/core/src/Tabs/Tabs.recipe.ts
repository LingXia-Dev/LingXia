export const tabsParts = {
  root: 'view',
  frame: 'view',
  list: 'view',
  track: 'view',
  indicator: 'view',
  tab: 'button',
  label: 'text',
  panel: 'view',
} as const;

/**
 * A row of equal-width tabs over one panel. The indicator is one box a tab wide that slides to the
 * chosen tab; reduced motion lands it at once. Keyboard focus is a fill on an underline tab, kept
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
  },
  /** Inside the frame's padding, so a tab's width is exactly a share of the list. */
  list: {
    base: 'grow relative flex-row',
  },
  track: {
    base: 'absolute inset-0 rtl:-scale-x-100',
    inner: 'absolute inset-0',
  },
  indicator: {
    base: 'absolute start-0 transition-transform duration-200 ease-spring',
    underline: '-bottom-[5px] h-0.5 rounded-full bg-accent',
  },
  tab: {
    base: 'grow shrink basis-0 min-w-0 flex-row items-center justify-center transition-colors duration-150 ease-out disabled:opacity-40',
    underline: 'h-control-md px-2 rounded-control focus-visible:bg-text/7',
  },
  /** A label too long for its share of the row is cut short rather than spilling into the next tab. */
  label: {
    base: 'font-medium truncate',
    underline: 'text-footnote',
    selected: 'text-text',
    unselected: 'text-muted',
  },
  panel: {
    base: '',
  },
} as const;
