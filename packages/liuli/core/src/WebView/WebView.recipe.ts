export const webViewParts = {
  root: 'view',
  webview: 'webview',
  expand: 'button',
  expandIcon: 'vector',
  panel: 'dialog',
  header: 'view',
  title: 'text',
  close: 'button',
  closeIcon: 'vector',
  stage: 'webview',
  footer: 'view',
} as const;

/**
 * A framed `<webview>` with a media-docked expand button and a modal sheet or full-screen dialog
 * that keeps the same live document mounted across both states.
 */
export const webViewRecipe = {
  root: {
    base: 'relative h-44 w-full overflow-hidden rounded-container border border-line bg-surface',
  },
  webview: {
    base: 'w-full h-full',
  },
  expand: {
    base: 'absolute end-2.5 top-2.5 size-9 items-center justify-center rounded-full bg-media text-on-media focus-visible:ring-2 focus-visible:ring-focus focus-visible:ring-offset-2',
  },
  expandIcon: {
    base: 'size-4',
  },
  panel: {
    base: 'w-full overflow-hidden transition duration-slow ease-spring motion-reduce:starting:opacity-0 motion-reduce:exit:opacity-0',
    mode: {
      sheet: 'mt-auto mb-0 max-w-[640px] h-[70vh] rounded-t-dialog border border-b-0 border-line bg-surface-overlay shadow-modal motion-safe:starting:translate-y-full motion-safe:exit:translate-y-full',
      full: 'm-0 h-full max-w-full rounded-none bg-canvas starting:opacity-0 exit:opacity-0',
    },
  },
  header: {
    base: 'flex-row items-center gap-3 ps-4 pe-2.5 py-2.5 border-b border-line',
  },
  title: {
    base: 'grow basis-0 text-title font-semibold tracking-tight text-text line-clamp-1',
  },
  close: {
    base: 'size-8 items-center justify-center rounded-full text-muted hover:bg-text/5 active:bg-text/8 focus-visible:ring-2 focus-visible:ring-focus focus-visible:ring-offset-2',
  },
  closeIcon: {
    base: 'size-5',
  },
  stage: {
    base: 'w-full grow basis-0 min-h-0',
  },
  footer: {
    base: 'flex-row flex-wrap items-center justify-end gap-2 px-4 py-3 border-t border-line',
  },
} as const;
