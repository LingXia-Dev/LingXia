import type {} from '@lingxia/bridge';
import type { NativeComponentMessage } from './types.js';
export type { NativeComponentMessage } from './types.js';

let warnedNoHandler = false;
const NATIVE_COMPONENT_LAYOUT_INVALIDATED_EVENT = "lingxia:native-component-layout-invalidated";
let pendingLayoutInvalidationFrame: number | null = null;
const pendingLayoutInvalidationTimers = new Map<number, number>();

/// Returns false when the native channel is not up yet and the message was
/// dropped, so callers that carry state (an island's first commit) can retry.
export function sendNativeComponentMessage(message: NativeComponentMessage): boolean {
  const sender =
    typeof window !== "undefined"
      ? window.LingXiaBridge?.nativeComponents?.send
      : undefined;
  if (typeof sender !== "function") {
    if (!warnedNoHandler) {
      warnedNoHandler = true;
      console.warn("[LingXia NativeComponent] message handler not available");
    }
    return false;
  }
  sender(message);
  return true;
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
    const send = nativeComponents?.send;
    // Always announce ready after the leaf handler is registered, including
    // Windows: island video queues play/playing until this id's handshake.
    if (typeof send === "function") {
      send({ action: "component.ready", id });
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
