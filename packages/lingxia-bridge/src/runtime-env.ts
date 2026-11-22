import type { BridgeConfig, PlatformOS } from './types';

export type { PlatformOS };

export type CommunicationMethod =
  | 'messageport'
  | 'jsinterface'
  | 'webmessage'
  | 'webkit'
  | 'unknown';

const PLATFORM_OS: readonly PlatformOS[] = ['ios', 'macos', 'android', 'windows', 'harmony'];

export const BRIDGE_CONFIG: BridgeConfig =
  (typeof window !== 'undefined' && window.__LX_BRIDGE_CFG) || {};

interface DisplayLanguageStore {
  value: string;
  /** Host revision of `value`; 0 for the value baked into the bridge config. */
  revision?: number;
  /** `value` narrowed to the languages the lxapp is written in. */
  content?: string;
  listeners: Set<(language: string) => void>;
}

const bootLanguage = () => BRIDGE_CONFIG.displayLanguage?.trim() || 'en-US';
const bootContent = () => BRIDGE_CONFIG.contentLanguage?.trim() || bootLanguage();

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
  value: bootLanguage(),
  content: bootContent(),
  listeners: new Set(),
};

function store(): DisplayLanguageStore {
  if (typeof window === 'undefined') return fallbackStore;
  if (!window.__lxDisplayLanguage) {
    window.__lxDisplayLanguage = {
      value: bootLanguage(),
      content: bootContent(),
      listeners: new Set(),
    };
  }
  return window.__lxDisplayLanguage;
}

/** Primary subtags of languages written right to left, for engines without `Intl.Locale#getTextInfo`. */
const RTL_LANGUAGES = new Set(['ar', 'ckb', 'dv', 'fa', 'he', 'ps', 'sd', 'ug', 'ur', 'yi']);

/** The writing direction of a BCP-47 tag. */
export function textDirection(tag: string): 'ltr' | 'rtl' {
  try {
    const locale = new Intl.Locale(tag) as Intl.Locale & {
      getTextInfo?: () => { direction?: string };
      textInfo?: { direction?: string };
    };
    const info = typeof locale.getTextInfo === 'function' ? locale.getTextInfo() : locale.textInfo;
    if (info?.direction === 'rtl' || info?.direction === 'ltr') return info.direction;
  } catch {
    // Not a tag `Intl` accepts; fall through to the subtag list.
  }
  return RTL_LANGUAGES.has(tag.split(/[-_]/)[0].toLowerCase()) ? 'rtl' : 'ltr';
}

/**
 * `lang` and `dir` on `<html>` follow the language the lxapp's text is in, so
 * a page never sets either: CSS logical properties and `:dir()` just work.
 */
function stampDocumentLanguage(): void {
  if (typeof document !== 'undefined' && document.documentElement) {
    const language = getContentLanguage();
    document.documentElement.lang = language;
    document.documentElement.dir = textDirection(language);
  }
}

stampDocumentLanguage();

/**
 * Host entry point for a language the user changed while this document was
 * open. Bootstrap alone would leave a live page in the language it started in,
 * with the native chrome around it already switched.
 *
 * The host pushes each change once, and again — the current value — when a
 * document's bridge reports ready, since a change made while it loaded found
 * no hook to call. Those two can land in either order, so a push carries the
 * host revision it took effect at and an older one never overwrites a newer
 * one. A push without a revision applies as it always did.
 */
function applyDisplayLanguage(next: unknown, revision?: unknown, content?: unknown): void {
  const normalized = typeof next === 'string' ? next.trim() : '';
  const narrowed = typeof content === 'string' ? content.trim() : '';
  const current = store();
  if (typeof revision === 'number' && Number.isFinite(revision)) {
    // The page's own push repeats the broadcast's revision to add the lxapp's narrowing.
    const held = current.revision ?? 0;
    if (revision < held || (revision === held && !narrowed)) return;
    current.revision = revision;
  }
  if (!normalized) return;
  const before = current.value;
  const beforeContent = getContentLanguage();
  // Without a narrowing, text that followed the display language keeps following it.
  const nextContent = narrowed || (beforeContent === before ? normalized : beforeContent);
  if (normalized === before && nextContent === beforeContent) return;
  current.value = normalized;
  current.content = nextContent;
  stampDocumentLanguage();
  for (const listener of [...current.listeners]) listener(normalized);
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

/** The display language narrowed to the `languages` the lxapp's lxapp.json lists. */
export function getContentLanguage(): string {
  const current = store();
  return current.content || current.value;
}

/**
 * Subscribe to host display-language changes. Returns an unsubscribe.
 *
 * Change-only, so that this composes with `useSyncExternalStore`: the listener
 * runs when the language changes, never on subscribe. Read the current value
 * with `getDisplayLanguage()`. Logic's `lx.host.displayLanguage.watch` differs
 * deliberately — it has no render loop to feed, so it delivers immediately.
 */
export function subscribeDisplayLanguage(
  listener: (language: string) => void,
): () => void {
  const listeners = store().listeners;
  listeners.add(listener);
  return () => {
    listeners.delete(listener);
  };
}

export function getPlatformOS(): PlatformOS {
  const os = String(BRIDGE_CONFIG.os ?? '').toLowerCase() as PlatformOS;
  return PLATFORM_OS.includes(os) ? os : 'unknown';
}

type HostClass = 'mobile' | 'desktop';

/**
 * Which kind of machine this is, mobile or desktop. Read through `isMobile()`
 * and `isDesktop()`; the class itself is host vocabulary, not lxapp API.
 *
 * Fixed for the life of the document. A shipped host is one machine; the
 * Runner re-serves the page when its simulated device changes class, so this
 * never has to change under a page that is already rendering.
 *
 * Hosts from before this config key shipped send nothing, so fall back to the
 * OS: every one of them is the machine it names.
 */
function hostClass(): HostClass {
  if (BRIDGE_CONFIG.hostClass === 'mobile' || BRIDGE_CONFIG.hostClass === 'desktop') {
    return BRIDGE_CONFIG.hostClass;
  }
  const os = getPlatformOS();
  return os === 'macos' || os === 'windows' ? 'desktop' : 'mobile';
}

export function isHarmony(): boolean {
  return getPlatformOS() === 'harmony';
}

export function isIOS(): boolean {
  return getPlatformOS() === 'ios';
}

export function isAndroid(): boolean {
  return getPlatformOS() === 'android';
}

export function isMacOS(): boolean {
  return getPlatformOS() === 'macos';
}

export function isWindows(): boolean {
  return getPlatformOS() === 'windows';
}

// Form factor, not OS: the Runner shows a real macOS/Windows build inside a
// phone frame, and a page that keyed off the OS would keep its desktop layout.
export function isDesktop(): boolean {
  return hostClass() === 'desktop';
}

export function isMobile(): boolean {
  return hostClass() === 'mobile';
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
  const os = getPlatformOS();
  if (os === 'ios' || os === 'macos') return 'webkit';
  if (os === 'harmony') return 'messageport';
  if (os === 'windows') return 'webmessage';
  if (os === 'android') {
    if (window.LingXiaProxy?.supportsMessagePort?.()) return 'messageport';
    return 'jsinterface';
  }
  return 'unknown';
}
