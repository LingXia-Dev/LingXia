import type { BridgeConfig } from './types';

export type CommunicationMethod = 'messageport' | 'jsinterface' | 'webkit' | 'unknown';
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

export function getCommunicationMethod(): CommunicationMethod {
  if (BRIDGE_CONFIG.os === 'iOS' || BRIDGE_CONFIG.os === 'macOS') return 'webkit';
  if (BRIDGE_CONFIG.os === 'Harmony') return 'messageport';
  if (BRIDGE_CONFIG.os === 'Android') {
    if (window.LingXiaProxy?.supportsMessagePort?.()) return 'messageport';
    return 'jsinterface';
  }
  return 'unknown';
}
