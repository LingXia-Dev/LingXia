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
 * A row of tabs over one panel: equal shares of the row, or each as wide as its label. No label is
 * ever cut: a tab is never narrower than its label, and a row that cannot hold them scrolls. The
 * indicator is one box a tab wide that slides to the chosen tab; reduced motion lands it at once.
 * Keyboard focus is a fill on an underline tab, kept 4 px above the indicator by the list's
 * padding. The segmented variant draws SegmentedControl's medium pieces (track, thumb, segment,
 * label), so the two never drift apart.
 */
export const tabsRecipe = {
  root: {
    base: 'gap-4',
  },
  frame: {
    base: '',
  },
  scroller: {
    base: 'grow overflow-x-auto',
    // A row that runs on under an edge fades there: past its end at rest, past both mid-scroll.
    more: {
      end: 'mask-[linear-gradient(to_left,transparent_0,#000_1.5rem)] rtl:mask-[linear-gradient(to_right,transparent_0,#000_1.5rem)]',
      start: 'mask-[linear-gradient(to_right,transparent_0,#000_1.5rem)] rtl:mask-[linear-gradient(to_left,transparent_0,#000_1.5rem)]',
      // Two layers, both kept: each fades one edge.
      both: 'mask-[linear-gradient(to_right,transparent_0,#000_1.5rem),linear-gradient(to_left,transparent_0,#000_1.5rem)]',
    },
  },
  /** At least the frame's width, so equal tabs share it and the rule spans it; wider, it scrolls. */
  list: {
    base: 'self-start min-w-full relative flex-row',
    // The rule is the list's, so the indicator can sit on it inside the scroll's clip.
    underline: 'pb-1 border-b border-text/8',
  },
  track: {
    base: 'absolute inset-0 rtl:-scale-x-100',
    inner: 'absolute inset-0',
  },
  indicator: {
    base: 'absolute start-0 transition-transform duration-base ease-spring',
    underline: '-bottom-[1px] h-0.5 rounded-full bg-accent',
  },
  tab: {
    base: 'flex-row items-center-safe justify-center transition-colors duration-fast ease-standard disabled:opacity-40',
    /** An equal share, but never narrower than its label. */
    equal: 'grow basis-0',
    content: 'shrink-0',
    underline: 'h-control-md rounded-control hover:bg-text/4 focus-visible:bg-text/7',
    pad: { equal: 'px-2', content: 'px-3' },
    /** Fills its tab, so the indicator can follow tabs of unequal width. */
    extent: 'absolute inset-0',
  },
  label: {
    base: 'font-medium whitespace-nowrap',
    underline: 'text-footnote',
    selected: 'text-text',
    unselected: 'text-muted',
  },
  panel: {
    base: '',
  },
} as const;
