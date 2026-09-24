/**
 * The lxapp's adaptive context, from the host: the same value Logic's
 * `lx.surface.watchContext()` delivers, so a View picks its layout without
 * Logic forwarding it through `setData`. It describes the lxapp's main
 * presentation; a page shown in an aside, float or window sees the same value.
 */
export interface SurfaceContext {
  /** `compact` (< 600 logical px) or `regular`, with hysteresis. */
  sizeClass: 'compact' | 'regular';
  /** The presentation's viewport width in logical pixels; 0 until measured. */
  width: number;
  /** The presentation's viewport height in logical pixels; 0 until measured. */
  height: number;
  /** Whether the host layout currently offers a docked aside. */
  aside: boolean;
}

/** What a host that sends nothing — one older than this API — is taken to say. */
const UNREPORTED: SurfaceContext = { sizeClass: 'compact', width: 0, height: 0, aside: false };

interface SurfaceContextStore {
  value: SurfaceContext;
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
    // Seeded by the host in this document's bridge config, before any script.
    const config = window.__LX_BRIDGE_CFG;
    window.__lxSurfaceContext = {
      value: parse(config?.surfaceContext) ?? UNREPORTED,
      revision: typeof config?.surfaceContextRevision === 'number' ? config.surfaceContextRevision : 0,
      listeners: new Set(),
    };
  }
  return window.__lxSurfaceContext;
}

const fallbackStore: SurfaceContextStore = { value: UNREPORTED, revision: 0, listeners: new Set() };

function parse(next: unknown): SurfaceContext | null {
  if (typeof next !== 'object' || next === null) return null;
  const { sizeClass, width, height, aside } = next as Record<string, unknown>;
  if (sizeClass !== 'compact' && sizeClass !== 'regular') return null;
  if (typeof width !== 'number' || typeof height !== 'number') return null;
  return { sizeClass, width, height, aside: aside === true };
}

function same(a: SurfaceContext, b: SurfaceContext): boolean {
  return a.sizeClass === b.sizeClass
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
 * The lxapp's adaptive context. The host seeds it into every page before any
 * script runs and pushes each change, so there is always a value; a host older
 * than this API reports `compact` with a 0×0 viewport.
 */
export function getSurfaceContext(): SurfaceContext {
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
