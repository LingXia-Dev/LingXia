import type { SplitViewContentWidth } from './SplitView';

export const splitViewParts = {
  root: 'view',
  sidebar: 'view',
  content: 'view',
  scroll: 'scroll',
  sidebarInner: 'view',
  contentInner: 'view',
} as const;

/**
 * A workspace: a fixed sidebar and the content beside it, each scrolling on its own, both on the
 * canvas and parted by a hairline. Content is capped so a line stays readable, and starts a fixed gutter
 * from the hairline, so a wider window adds room on the far side.
 */
export const splitViewRecipe = {
  root: {
    base: 'grow flex-row bg-canvas',
  },
  sidebar: {
    base: 'shrink-0 border-e border-text/8',
  },
  sidebarInner: {
    base: 'px-3 py-4 gap-4',
  },
  content: {
    base: 'grow shrink basis-0',
  },
  /** Each pane scrolls on its own. */
  scroll: {
    base: 'grow',
  },
  contentInner: {
    base: 'w-full self-start ps-10 pe-8 pt-6 pb-12 gap-8',
    width: { prose: 'max-w-[784px]', wide: 'max-w-[1024px]', full: '' } satisfies Record<SplitViewContentWidth, string>,
  },
} as const;
