import type { NativeComponentMessage } from './types.js';
export type { NativeComponentMessage };

let warnedNoHandler = false;

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
    const unregister = registerFn(id, handler);
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
