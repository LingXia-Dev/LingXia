import type { SameLevelMessage } from './types.js';
export type { SameLevelMessage };

let warnedNoHandler = false;

export function sendSameLevelMessage(message: SameLevelMessage) {
  const sender =
    typeof window !== "undefined"
      ? window.LingXiaBridge?.sameLevel?.send
      : undefined;
  if (typeof sender !== "function") {
    if (!warnedNoHandler) {
      warnedNoHandler = true;
      console.warn("[LingXia SameLevel] message handler not available");
    }
    return;
  }
  sender(message);
}

export function registerSameLevelHandler(
  id: string,
  handler: (msg: SameLevelMessage) => void
): () => void {
  const registerFn =
    typeof window !== "undefined"
      ? window.LingXiaBridge?.sameLevel?.register
      : undefined;
  if (typeof registerFn === "function") {
    return registerFn(id, handler);
  }
  if (!warnedNoHandler) {
    warnedNoHandler = true;
    console.warn("[LingXia SameLevel] message handler not available");
  }
  return () => {};
}
