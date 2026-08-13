export interface PageChromeRect {
  readonly width: number;
  readonly height: number;
  readonly top: number;
  readonly right: number;
  readonly bottom: number;
  readonly left: number;
}

export interface PageChromeLayoutSnapshot {
  readonly revision: number;
  /**
   * Height of the runtime-owned drag strip across the top of a
   * `chrome: 'full'` window. Zero everywhere else. Lay content out beneath it
   * the same way you already do for the capsule — the strip stays draggable
   * whether or not the page cooperates.
   */
  readonly topInset: number;
  readonly bottomInset: number;
  readonly capsuleRect: PageChromeRect | null;
  readonly capsuleInlineEndInset: number;
}

export interface LxPageChrome {
  readonly layout: PageChromeLayoutSnapshot;
}

export type PageChromeLayoutListener = (
  layout: PageChromeLayoutSnapshot,
) => void;

declare global {
  interface Window {
    readonly lxPageChrome: LxPageChrome;
  }

  interface WindowEventMap {
    lxpagechromechange: CustomEvent<PageChromeLayoutSnapshot>;
  }
}

const initialLayout = Object.freeze<PageChromeLayoutSnapshot>({
  revision: 0,
  topInset: 0,
  bottomInset: 0,
  capsuleRect: null,
  capsuleInlineEndInset: 0,
});

export function shouldApplyPageChromeRevision(
  currentRevision: number,
  nextRevision: number,
): boolean {
  return nextRevision >= currentRevision;
}

function projectPageChromeLayout(layout: PageChromeLayoutSnapshot): void {
  const root = document.documentElement;
  root?.style.setProperty(
    "--lx-page-chrome-top-inset",
    `${layout.topInset}px`,
  );
  root?.style.setProperty(
    "--lx-page-chrome-bottom-inset",
    `${layout.bottomInset}px`,
  );
  root?.style.setProperty(
    "--lx-page-chrome-capsule-inline-end-inset",
    `${layout.capsuleInlineEndInset}px`,
  );
}

/** Read the latest realized page-chrome layout synchronously. */
export function getPageChromeLayout(): PageChromeLayoutSnapshot {
  if (typeof window === "undefined") return initialLayout;
  return installPageChromeRuntime()?.layout ?? initialLayout;
}

/** Subscribe to realized page-chrome layout changes. */
export function subscribePageChromeLayout(
  listener: PageChromeLayoutListener,
): () => void {
  if (typeof window === "undefined") return () => {};
  installPageChromeRuntime();
  const handleChange = (event: CustomEvent<PageChromeLayoutSnapshot>) => {
    listener(event.detail);
  };
  window.addEventListener("lxpagechromechange", handleChange);
  return () => window.removeEventListener("lxpagechromechange", handleChange);
}

/** Ensure browser previews have the same synchronous contract as native pages. */
export function installPageChromeRuntime(): LxPageChrome | undefined {
  if (typeof window === "undefined") return undefined;
  if (window.lxPageChrome) return window.lxPageChrome;

  let layout = initialLayout;
  const api = Object.freeze({
    get layout() {
      return layout;
    },
  });
  Object.defineProperty(window, "lxPageChrome", {
    configurable: false,
    enumerable: true,
    value: api,
  });

  Object.defineProperty(window, "__lingxiaApplyPageChrome", {
    configurable: false,
    enumerable: false,
    value(next: PageChromeLayoutSnapshot, scheme: "light" | "dark") {
      if (!shouldApplyPageChromeRevision(layout.revision, next.revision)) return;
      const capsuleRect = next.capsuleRect
        ? Object.freeze({ ...next.capsuleRect })
        : null;
      layout = Object.freeze({ ...next, capsuleRect });
      projectPageChromeLayout(layout);
      const root = document.documentElement;
      if (root) {
        root.style.colorScheme = scheme;
        // Authoritative theme signal: platform media queries can lag an
        // in-place appearance switch (Android locks them at webview
        // creation); [data-theme] CSS keys off this instead.
        root.setAttribute('data-theme', scheme);
      }
      window.dispatchEvent(
        new CustomEvent("lxpagechromechange", { detail: layout }),
      );
    },
  });
  projectPageChromeLayout(initialLayout);
  return api;
}

declare global {
  interface Window {
    readonly __lingxiaApplyPageChrome?: (
      layout: PageChromeLayoutSnapshot,
      scheme: "light" | "dark",
    ) => void;
  }
}

installPageChromeRuntime();
