import type { TabsVariant } from './Tabs';

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
 * chosen tab; reduced motion lands it at once.
 */
export const tabsRecipe = {
  root: {
    base: 'gap-4',
  },
  frame: {
    base: '',
    variant: {
      underline: 'border-b border-line',
      segmented: 'p-0.5 rounded-control bg-surface-sunken',
    } satisfies Record<TabsVariant, string>,
  },
  /** Inside the frame's padding, so a tab's width is exactly a share of the list. */
  list: {
    base: 'relative flex-row',
  },
  track: {
    base: 'absolute inset-0 rtl:-scale-x-100',
    inner: 'absolute inset-0',
  },
  indicator: {
    base: 'absolute start-0 transition-transform duration-200 ease-spring',
    variant: {
      underline: '-bottom-px h-0.5 rounded-full bg-accent',
      segmented: 'inset-y-0 rounded-segment border border-text/6 bg-surface-raised shadow-contact',
    } satisfies Record<TabsVariant, string>,
  },
  tab: {
    base: 'grow shrink basis-0 flex-row items-center justify-center px-3 transition-colors duration-150 ease-out disabled:opacity-40',
    variant: {
      underline: 'h-control-md',
      segmented: 'h-8 rounded-segment',
    } satisfies Record<TabsVariant, string>,
  },
  label: {
    base: 'text-footnote font-medium',
    selected: 'text-text',
    unselected: 'text-muted',
  },
  panel: {
    base: '',
  },
} as const;
