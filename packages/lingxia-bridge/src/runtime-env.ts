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
