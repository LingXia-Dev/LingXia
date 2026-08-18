import type { BridgeConfig } from './types';

export type CommunicationMethod =
  | 'messageport'
  | 'jsinterface'
  | 'webmessage'
  | 'webkit'
  | 'unknown';
export type PlatformOS = NonNullable<BridgeConfig['os']> | 'unknown';

export const BRIDGE_CONFIG: BridgeConfig =
  (typeof window !== 'undefined' && window.__LX_BRIDGE_CFG) || {};

interface DisplayLanguageStore {
  value: string;
  listeners: Set<() => void>;
}

/**
 * One store per document, deliberately on `window`.
 *
 * This module is present twice in a page: once as the global bridge runtime
 * the host injects, once bundled into the page's own JS. Module-local state
 * would give each copy its own value — the host would push the change into
 * whichever copy won the race to install the hook, and the other, which is the
 * one the framework hooks read, would answer with the boot value forever.
 */
const fallbackStore: DisplayLanguageStore = {
  value: BRIDGE_CONFIG.displayLanguage?.trim() || 'en-US',
  listeners: new Set(),
};

function store(): DisplayLanguageStore {
  if (typeof window === 'undefined') return fallbackStore;
  if (!window.__lxDisplayLanguage) {
    window.__lxDisplayLanguage = {
      value: BRIDGE_CONFIG.displayLanguage?.trim() || 'en-US',
      listeners: new Set(),
    };
  }
  return window.__lxDisplayLanguage;
}

function stampDocumentLanguage(): void {
  if (typeof document !== 'undefined' && document.documentElement) {
    document.documentElement.lang = store().value;
  }
}

stampDocumentLanguage();

/**
 * Host entry point for a language the user changed while this document was
 * open. Bootstrap alone would leave a live page in the language it started in,
 * with the native chrome around it already switched.
 */
function applyDisplayLanguage(next: unknown): void {
  const normalized = typeof next === 'string' ? next.trim() : '';
  const current = store();
  if (!normalized || normalized === current.value) return;
  current.value = normalized;
  stampDocumentLanguage();
  for (const listener of [...current.listeners]) listener();
}

if (typeof window !== 'undefined' && !window.__lingxiaApplyDisplayLanguage) {
  Object.defineProperty(window, '__lingxiaApplyDisplayLanguage', {
    configurable: false,
    enumerable: false,
    value: applyDisplayLanguage,
  });
}

export function getDisplayLanguage(): string {
  return store().value;
}

/** Subscribe to host display-language changes. Returns an unsubscribe. */
export function subscribeDisplayLanguage(listener: () => void): () => void {
  const listeners = store().listeners;
  listeners.add(listener);
  return () => {
    listeners.delete(listener);
  };
}

export function getPlatformOS(): PlatformOS {
  return BRIDGE_CONFIG.os || 'unknown';
}

export function isHarmony(): boolean {
  return BRIDGE_CONFIG.os === 'Harmony';
}

export function isIOS(): boolean {
  return BRIDGE_CONFIG.os === 'iOS';
}

export function isAndroid(): boolean {
  return BRIDGE_CONFIG.os === 'Android';
}

export function isMacOS(): boolean {
  return BRIDGE_CONFIG.os === 'macOS';
}

export function isWindows(): boolean {
  return BRIDGE_CONFIG.os === 'Windows';
}

export function isDesktop(): boolean {
  return isMacOS() || isWindows();
}

// iOS and macOS share the WKWebView transport, so features scoped to it (e.g.
// the streaming downstream) key off this rather than the two OS checks.
export function isApple(): boolean {
  return isIOS() || isMacOS();
}

// True when attached to a `lingxia dev` session (the host sets `dev` in
// `__LX_BRIDGE_CFG`). Used to surface the bridge's own protocol/lifecycle trace
// only during development.
export function isDevSession(): boolean {
  return BRIDGE_CONFIG.dev === true;
}

// True when running inside the LingXia Runner (the `lingxia dev` device
// simulator), which the host marks in `__LX_BRIDGE_CFG`. Unlike a real host
// app in dev mode, the Runner lacks host-declared surfaces such as the
// terminal — apps read this to hide those affordances.
export function isRunner(): boolean {
  return BRIDGE_CONFIG.runner === true;
}

export function getCommunicationMethod(): CommunicationMethod {
  if (BRIDGE_CONFIG.os === 'iOS' || BRIDGE_CONFIG.os === 'macOS') return 'webkit';
  if (BRIDGE_CONFIG.os === 'Harmony') return 'messageport';
  if (BRIDGE_CONFIG.os === 'Windows') return 'webmessage';
  if (BRIDGE_CONFIG.os === 'Android') {
    if (window.LingXiaProxy?.supportsMessagePort?.()) return 'messageport';
    return 'jsinterface';
  }
  return 'unknown';
}
