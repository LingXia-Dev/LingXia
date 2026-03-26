import type {} from '@lingxia/bridge';

export function isHarmony(): boolean {
  return window.LingXiaBridge?.platform?.isHarmony?.() === true;
}

export function isIOS(): boolean {
  return window.LingXiaBridge?.platform?.isIOS?.() === true;
}

export function isAndroid(): boolean {
  return window.LingXiaBridge?.platform?.isAndroid?.() === true;
}

export function isMacOS(): boolean {
  return window.LingXiaBridge?.platform?.isMacOS?.() === true;
}

export function isDesktop(): boolean {
  return window.LingXiaBridge?.platform?.isDesktop?.() === true;
}

export function getOS(): string {
  return window.LingXiaBridge?.platform?.getOS?.() ?? "unknown";
}
