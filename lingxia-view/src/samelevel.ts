export type SameLevelMessage = {
  action: string;
  id: string;
  type?: string;
  event?: string;
  detail?: any;
  payload?: any;
  rect?: { x: number; y: number; width: number; height: number };
  props?: Record<string, unknown>;
  zIndex?: number;
  cornerRadius?: number;
};

declare global {
  interface Window {
    LingXiaBridge?: {
      sameLevel?: {
        send?: (msg: SameLevelMessage) => void;
        register?: (id: string, handler: (msg: SameLevelMessage) => void) => () => void;
      };
    };
  }
}

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
