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
  const registerFn =
    typeof window !== "undefined"
      ? window.LingXiaBridge?.nativeComponents?.register
      : undefined;
  if (typeof registerFn === "function") {
    return registerFn(id, handler);
  }
  if (!warnedNoHandler) {
    warnedNoHandler = true;
    console.warn("[LingXia NativeComponent] message handler not available");
  }
  return () => {};
}
