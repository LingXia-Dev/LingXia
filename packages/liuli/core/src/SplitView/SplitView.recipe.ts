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
    // The content starts 16 pt in; the button's border and padding (13 pt) and the chevron's own
    // inset in its box (a third of 16 pt) would start the stroke 2.33 pt further, so the row pulls it out.
    base: 'flex-row pt-1 -ms-[2.33px]',
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
} as const;
