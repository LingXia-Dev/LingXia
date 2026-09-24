import {
  BRIDGE_CONFIG,
  getDisplayLanguage,
  getPlatformOS,
  isDesktop,
  subscribeDisplayLanguage,
  type PlatformOS,
} from './runtime-env';
import { getSurfaceContext, subscribeSurfaceContext } from './surface-context';

/**
 * What the host decided for this page: the facts a View picks its layout and
 * copy from. Every field changes rarely or never, so reading them costs a
 * re-render only when one of them actually changes — never while a window is
 * dragged. Measured geometry belongs to CSS (`--lx-page-chrome-*`, container
 * queries), not here.
 */
export interface LxHost {
  /** `compact` (< 600 logical px) or `regular`, with hysteresis; the same value Logic's `lx.surface.watchContext()` reports. */
  readonly sizeClass: 'compact' | 'regular';
  /** Whether the host layout currently offers a docked aside. */
  readonly aside: boolean;
  /** The BCP-47 tag in effect, as Logic's `lx.host.displayLanguage.get()`. */
  readonly displayLanguage: string;
  /** Which kind of machine this is; fixed for the document. A narrowed desktop window is still `desktop`. */
  readonly formFactor: 'mobile' | 'desktop';
  /** The operating system, for the rare OS-specific feature; not a layout fact. */
  readonly os: PlatformOS;
  /** Whether the page runs in the LingXia Runner (the `lingxia dev` simulator). */
  readonly runner: boolean;
}

let current: LxHost | null = null;

function read(): LxHost {
  const surface = getSurfaceContext();
  return {
    sizeClass: surface.sizeClass,
    aside: surface.aside,
    displayLanguage: getDisplayLanguage(),
    formFactor: isDesktop() ? 'desktop' : 'mobile',
    os: getPlatformOS(),
    runner: BRIDGE_CONFIG.runner === true,
  };
}

function same(a: LxHost, b: LxHost): boolean {
  return a.sizeClass === b.sizeClass
    && a.aside === b.aside
    && a.displayLanguage === b.displayLanguage
    && a.formFactor === b.formFactor
    && a.os === b.os
    && a.runner === b.runner;
}

/**
 * The host facts for this page. The same object is returned until a field
 * changes, so it composes with `useSyncExternalStore` and with memoization.
 */
export function getHost(): LxHost {
  const next = read();
  if (current === null || !same(current, next)) current = next;
  return current;
}

/**
 * Follow the host facts. Change-only: the listener runs when a field of
 * `getHost()` changes — not on subscribe, and not when only the viewport's
 * size moves within its size class. Returns an unsubscribe.
 */
export function subscribeHost(listener: () => void): () => void {
  let last = getHost();
  const check = () => {
    const next = getHost();
    if (next === last) return;
    last = next;
    listener();
  };
  const stopLanguage = subscribeDisplayLanguage(check);
  const stopSurface = subscribeSurfaceContext(check);
  return () => {
    stopLanguage();
    stopSurface();
  };
}
