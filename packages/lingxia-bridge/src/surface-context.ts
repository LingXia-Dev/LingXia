/**
 * The page's adaptive context, pushed by the host: the same value Logic's
 * `lx.surface.watchContext()` delivers, so a View picks its layout without
 * Logic forwarding it through `setData`.
 */
export interface SurfaceContext {
  /** `compact` (< 600 logical px) or `regular`, with hysteresis. */
  sizeClass: 'compact' | 'regular';
  /** The page's viewport width in logical pixels. */
  width: number;
  /** The page's viewport height in logical pixels. */
  height: number;
  /** Whether the host layout currently offers a docked aside. */
  aside: boolean;
}

interface SurfaceContextStore {
  value: SurfaceContext | null;
  /** The newest host revision applied; an older push arriving late is stale. */
  revision: number;
  listeners: Set<() => void>;
}

/**
 * One store per document, on `window`, for the same reason as the display
 * language: the bridge module is present twice in a page, and the host must
 * reach the copy the framework hooks read.
 */
function store(): SurfaceContextStore {
  if (typeof window === 'undefined') return fallbackStore;
  if (!window.__lxSurfaceContext) {
    window.__lxSurfaceContext = { value: null, revision: 0, listeners: new Set() };
  }
  return window.__lxSurfaceContext;
}

const fallbackStore: SurfaceContextStore = { value: null, revision: 0, listeners: new Set() };

function parse(next: unknown): SurfaceContext | null {
  if (typeof next !== 'object' || next === null) return null;
  const { sizeClass, width, height, aside } = next as Record<string, unknown>;
  if (sizeClass !== 'compact' && sizeClass !== 'regular') return null;
  if (typeof width !== 'number' || typeof height !== 'number') return null;
  return { sizeClass, width, height, aside: aside === true };
}

function same(a: SurfaceContext | null, b: SurfaceContext): boolean {
  return a !== null
    && a.sizeClass === b.sizeClass
    && a.width === b.width
    && a.height === b.height
    && a.aside === b.aside;
}

/**
 * Host entry point: once the page's bridge is up, and on every change. Pushes
 * can run out of order across host threads, so each carries a revision and an
 * older one than the store holds is dropped.
 */
function applySurfaceContext(next: unknown, revision?: unknown): void {
  const context = parse(next);
  const current = store();
  if (!context) return;
  if (typeof revision === 'number') {
    if (revision <= current.revision) return;
    current.revision = revision;
  }
  if (same(current.value, context)) return;
  // A new object per change, so `useSyncExternalStore` sees it changed.
  current.value = context;
  for (const listener of [...current.listeners]) listener();
}

if (typeof window !== 'undefined' && !window.__lingxiaApplySurfaceContext) {
  Object.defineProperty(window, '__lingxiaApplySurfaceContext', {
    configurable: false,
    enumerable: false,
    value: applySurfaceContext,
  });
}

/**
 * The page's adaptive context, or `null` until the host has sent it — which it
 * does as soon as the page's bridge is up, ahead of the page's first data.
 */
export function getSurfaceContext(): SurfaceContext | null {
  return store().value;
}

/**
 * Follow the adaptive context. Change-only, like `subscribeDisplayLanguage`,
 * so it composes with `useSyncExternalStore`. Returns an unsubscribe.
 */
export function subscribeSurfaceContext(listener: () => void): () => void {
  const listeners = store().listeners;
  listeners.add(listener);
  return () => {
    listeners.delete(listener);
  };
}
