import type { SplitViewContentWidth } from './SplitView';

export const splitViewParts = {
  root: 'view',
  sidebar: 'view',
  content: 'view',
  pane: 'view',
  back: 'view',
  scroll: 'scroll',
  sidebarInner: 'view',
  contentInner: 'view',
  actions: 'view',
} as const;

/**
 * A workspace: a fixed sidebar and the content beside it, each scrolling on its own, both on the
 * canvas and parted by a hairline. Content is capped so a line stays readable, and starts a fixed gutter
 * from the hairline, so a wider window adds room on the far side. On a compact screen one pane takes
 * the whole width inside the page gutter, the content under its back button.
 */
export const splitViewRecipe = {
  root: {
    base: 'grow bg-canvas',
    regular: 'flex-row',
    // A pane that slides is cut at the split view's own edge.
    compact: 'min-h-0 overflow-hidden',
  },
  /**
   * Compact: the pane on show. The content is pushed in from the end edge and leaves the same way;
   * the sidebar gives way a quarter of its width, and comes back from there. Reduced motion changes
   * panes at once.
   */
  pane: {
    base: 'grow shrink basis-0 bg-canvas transition-transform duration-slow ease-standard',
    content: 'motion-safe:starting:translate-x-full motion-safe:exit:translate-x-full rtl:motion-safe:starting:-translate-x-full rtl:motion-safe:exit:-translate-x-full',
    sidebar: 'motion-safe:exit:-translate-x-1/4 rtl:motion-safe:exit:translate-x-1/4',
    // Only a sidebar that is come back to enters; the first one is simply there.
    returning: 'motion-safe:starting:-translate-x-1/4 rtl:motion-safe:starting:translate-x-1/4',
  },
  sidebar: {
    base: 'shrink-0 border-e border-text/8',
  },
  sidebarInner: {
    base: 'gap-4',
    regular: 'px-3 py-4',
    compact: 'px-4 pt-2 pb-8',
  },
  /** Compact: the way back to the sidebar, over the content and outside its scroll. */
  back: {
    // The pressed pill keeps 8 pt clear of the pane's edges; the chevron's stroke still starts on the
    // content's 16 pt edge: 8 + border 1 + padding 1.67 + the chevron's own inset in its box (5.33).
    base: 'flex-row ps-2 pt-2',
    button: 'ps-[1.67px]',
  },
  content: {
    base: 'grow shrink basis-0',
  },
  /** Each pane scrolls on its own. */
  scroll: {
    base: 'grow',
  },
  contentInner: {
    base: 'w-full self-start',
    regular: 'ps-10 pe-8 pt-6 pb-12 gap-8',
    compact: 'px-4 pt-2 pb-8 gap-6',
    width: { prose: 'max-w-[784px]', wide: 'max-w-[1024px]', full: '' } satisfies Record<SplitViewContentWidth, string>,
  },
  /** The content's actions, under its scroll behind a hairline, starting on the content's own edge. */
  actions: {
    base: 'flex-row flex-wrap items-center gap-2 border-t border-text/8 bg-canvas',
    regular: 'ps-10 pe-8 py-3',
    compact: 'px-4 pt-3 pb-1',
  },
} as const;
