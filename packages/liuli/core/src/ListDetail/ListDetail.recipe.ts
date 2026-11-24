import type { ListDetailContentWidth } from './ListDetail';

export const listDetailParts = {
  root: 'view',
  list: 'view',
  detail: 'view',
  back: 'view',
  scroll: 'scroll',
  listInner: 'view',
  detailInner: 'view',
  placeholder: 'view',
  actions: 'view',
} as const;

/**
 * A list and its detail on the canvas, parted by a hairline, each scrolling on its own. The detail is
 * capped so a line stays readable, and starts a fixed gutter from the hairline, so a wider window adds
 * room on the far side. On a phone one pane takes the width inside the page gutter, the detail under its
 * way back.
 */
export const listDetailRecipe = {
  root: {
    base: 'grow min-h-0 bg-canvas',
    regular: 'flex-row',
    // A pane that slides is cut at the root's own edge.
    compact: 'overflow-hidden',
  },
  /**
   * The list. On a phone it stays mounted while the detail covers it, so its scroll survives; it gives
   * way a quarter of its width and comes back from there. Reduced motion changes panes at once.
   */
  list: {
    base: 'min-h-0 bg-canvas',
    regular: 'shrink-0 border-e border-text/8',
    compact: 'grow shrink basis-0 transition-transform duration-slow ease-standard',
    hidden: 'hidden',
    returning: 'motion-safe:starting:-translate-x-1/4 rtl:motion-safe:starting:translate-x-1/4',
  },
  /** The detail. On a phone it is pushed in from the end edge and leaves the same way. */
  detail: {
    base: 'grow shrink basis-0 min-w-0 min-h-0 bg-canvas',
    compact: 'transition-transform duration-slow ease-standard motion-safe:starting:translate-x-full motion-safe:exit:translate-x-full rtl:motion-safe:starting:-translate-x-full rtl:motion-safe:exit:-translate-x-full',
  },
  /** Phone: the way back, over the detail and outside its scroll. */
  back: {
    // The pressed pill keeps 8 pt clear of the pane's edges; the chevron's stroke still starts on the
    // content's 16 pt edge: 8 + border 1 + padding 1.67 + the chevron's own inset in its box (5.33).
    base: 'shrink-0 flex-row items-center gap-2 ps-2 pe-4 pt-2',
    button: 'ps-[1.67px]',
  },
  /** Each pane scrolls on its own. */
  scroll: {
    base: 'grow',
  },
  listInner: {
    base: 'gap-4',
    regular: 'px-3 py-4',
    compact: 'px-4 pt-2 pb-8',
  },
  detailInner: {
    base: 'w-full self-start',
    regular: 'ps-10 pe-8 pt-6 pb-12 gap-8',
    compact: 'px-4 pt-2 pb-8 gap-6',
    width: { prose: 'max-w-[784px]', wide: 'max-w-[1024px]', full: '' } satisfies Record<ListDetailContentWidth, string>,
  },
  /** Wide screen, nothing chosen: the placeholder, centred in the pane. */
  placeholder: {
    base: 'grow items-center justify-center p-8',
  },
  /** The record's actions, under its scroll behind a hairline, starting on the detail's own edge. */
  actions: {
    base: 'flex-row flex-wrap items-center gap-2 border-t border-text/8 bg-canvas',
    regular: 'ps-10 pe-8 py-3',
    compact: 'px-4 pt-3 pb-1',
  },
} as const;
