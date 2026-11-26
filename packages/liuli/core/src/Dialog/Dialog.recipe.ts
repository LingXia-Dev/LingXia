import type { DialogSize } from './Dialog';

export const dialogParts = {
  panel: 'dialog',
  media: 'view',
  mediaClose: 'button',
  header: 'view',
  title: 'text',
  headerAction: 'view',
  close: 'button',
  closeIcon: 'vector',
  description: 'text',
  body: 'scroll',
  footer: 'view',
  failed: 'view',
  failedIcon: 'vector',
  failedText: 'text',
} as const;

/**
 * A centred modal panel. It enters and leaves by fading and scaling from 95%; under reduced motion
 * it only fades. The backdrop is the primitive's `bg-scrim`.
 */
export const dialogRecipe = {
  panel: {
    base: 'p-5 gap-4 transition duration-base ease-standard starting:opacity-0 exit:opacity-0',
    /** A floating panel sits on the overlay surface and rises 8 px into place; it holds a scroll, which the View never scales. */
    floating: 'w-[90%] max-h-[85vh] rounded-dialog border border-line bg-surface-overlay shadow-modal motion-safe:starting:translate-y-2 motion-safe:exit:translate-y-2',
    /** `full` has nothing behind it to float over: it is a page, on the page's canvas, and only fades. */
    size: { sm: 'max-w-90', md: 'max-w-120', lg: 'max-w-160', full: 'w-full h-full rounded-none bg-canvas' } satisfies Record<DialogSize, string>,
  },
  /** The picture above the title: it undoes the panel's padding to reach three edges, and takes the top corners. */
  media: {
    base: 'relative -mx-5 -mt-5 overflow-hidden',
    floating: 'rounded-t-dialog',
    picture: 'self-stretch',
  },
  /** Over a picture the close button brings its own dock, so it reads on any image. */
  mediaClose: {
    base: 'absolute end-3 top-3 size-8 items-center justify-center rounded-full bg-media text-on-media focus-visible:ring-2 focus-visible:ring-focus focus-visible:ring-offset-2',
  },
  header: {
    base: 'flex-row items-start gap-3',

    /** No title drawn: the close button and any header action keep to the end. */
    bare: 'justify-end',
  },
  title: {
    base: 'grow basis-0 text-title font-semibold tracking-tight text-text',
  },
  /** One small control beside the title, on the close button's line. */
  headerAction: {
    base: 'shrink-0 flex-row items-center gap-1 min-h-8 -mt-1',
  },
  close: {
    base: 'size-8 -me-1.5 -mt-1 items-center justify-center rounded-full text-muted hover:bg-text/5 active:bg-text/8 focus-visible:ring-2 focus-visible:ring-focus focus-visible:ring-offset-2',
  },
  closeIcon: {
    base: 'size-5',
  },
  description: {
    base: 'text-body text-muted whitespace-pre-line',
  },
  body: {
    base: 'shrink min-h-0 gap-3 -mx-5 px-5',
    /** In a full dialog the body takes the height the header and footer leave. */
    full: 'grow',
  },
  footer: {
    base: 'flex-row flex-wrap justify-end gap-2',
  },
  /**
   * An AlertDialog's failed action, in the body above the buttons: the words in the body colour, which
   * reads on every surface, marked by the danger glyph.
   */
  failed: {
    base: 'flex-row items-start gap-2',
  },
  failedIcon: {
    base: 'size-5 shrink-0 text-danger',
  },
  failedText: {
    base: 'flex-1 text-body text-text',
  },
} as const;
