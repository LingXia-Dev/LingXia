import {
  BRIDGE_CONFIG,
  getLanguage,
  getPlatformOS,
  isDesktop,
  subscribeLanguage,
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
  /** `compact` (< 600 logical px) or `regular`, with hysteresis; the same value Logic's `page.env` reports. */
  readonly sizeClass: 'compact' | 'regular';
  /** Whether the window currently has a dock region for `as: 'aside'`; not where this page is shown. */
  readonly canDock: boolean;
  /** The language the page's text is in: the host's display language narrowed to the `languages` in lxapp.json. */
  readonly language: string;
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
    canDock: surface.canDock,
    language: getLanguage(),
    formFactor: isDesktop() ? 'desktop' : 'mobile',
    os: getPlatformOS(),
    runner: BRIDGE_CONFIG.runner === true,
  };
}

function same(a: LxHost, b: LxHost): boolean {
  return a.sizeClass === b.sizeClass
    && a.canDock === b.canDock
    && a.language === b.language
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
  const stopLanguage = subscribeLanguage(check);
  const stopSurface = subscribeSurfaceContext(check);
  return () => {
    stopLanguage();
    stopSurface();
  };
}
