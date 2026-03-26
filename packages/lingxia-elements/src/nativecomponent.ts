import type {} from '@lingxia/bridge';
import type { NativeComponentMessage } from './types.js';
export type { NativeComponentMessage } from './types.js';

let warnedNoHandler = false;
const NATIVE_COMPONENT_LAYOUT_INVALIDATED_EVENT = "lingxia:native-component-layout-invalidated";
let pendingLayoutInvalidationFrame: number | null = null;
const pendingLayoutInvalidationTimers = new Map<number, number>();

export function sendNativeComponentMessage(message: NativeComponentMessage) {
  const sender =
    typeof window !== "undefined"
      ? window.LingXiaBridge?.nativeComponents?.send
      : undefined;
  if (typeof sender !== "function") {
    if (!warnedNoHandler) {
      warnedNoHandler = true;
      console.warn("[LingXia NativeComponent] message handler not available");
    }
    return;
  }
  sender(message);
}

export function registerNativeComponentHandler(
  id: string,
  handler: (msg: NativeComponentMessage) => void
): () => void {
  const nativeComponents =
    typeof window !== "undefined"
      ? window.LingXiaBridge?.nativeComponents
      : undefined;
  const registerFn = nativeComponents?.register;
  if (typeof registerFn === "function") {
    const unregister = registerFn(id, handler as Parameters<typeof registerFn>[1]);
    const platform = window.LingXiaBridge?.platform;
    const requiresReadyHandshake = !!(
      platform?.isIOS?.() ||
      platform?.isAndroid?.() ||
      platform?.isMacOS?.()
    );
    const hasHandler = nativeComponents?.hasHandler;
    const send = nativeComponents?.send;
    if (typeof send === "function") {
      const nativeReady = typeof hasHandler === "function" ? hasHandler() : true;
      if (requiresReadyHandshake || nativeReady) {
        send({ action: "component.ready", id });
      }
    }
    return unregister;
  }
  if (!warnedNoHandler) {
    warnedNoHandler = true;
    console.warn("[LingXia NativeComponent] message handler not available");
  }
  return () => {};
}

function dispatchNativeComponentLayoutInvalidated(): void {
  if (typeof window === "undefined") return;
  window.dispatchEvent(new CustomEvent(NATIVE_COMPONENT_LAYOUT_INVALIDATED_EVENT));
}

export function invalidateNativeComponentLayout(delays: number[] = [32, 96]): void {
  if (typeof window === "undefined") return;
  if (pendingLayoutInvalidationFrame === null) {
    pendingLayoutInvalidationFrame = window.requestAnimationFrame(() => {
      pendingLayoutInvalidationFrame = null;
      dispatchNativeComponentLayoutInvalidated();
    });
  }
  const uniqueDelays = new Set(
    delays
      .map((delay) => Math.round(delay))
      .filter((delay) => delay > 0)
  );
  uniqueDelays.forEach((delay) => {
    if (pendingLayoutInvalidationTimers.has(delay)) {
      return;
    }
    const timer = window.setTimeout(() => {
      pendingLayoutInvalidationTimers.delete(delay);
      dispatchNativeComponentLayoutInvalidated();
    }, delay);
    pendingLayoutInvalidationTimers.set(delay, timer);
  });
}

export function addNativeComponentLayoutInvalidationListener(
  listener: EventListenerOrEventListenerObject
): () => void {
  if (typeof window === "undefined") return () => {};
  window.addEventListener(NATIVE_COMPONENT_LAYOUT_INVALIDATED_EVENT, listener);
  return () => {
    window.removeEventListener(NATIVE_COMPONENT_LAYOUT_INVALIDATED_EVENT, listener);
  };
}
