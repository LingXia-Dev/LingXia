/**
 * Shared type definitions for LingXia View.
 */

export type NativeComponentMessage = {
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
      platform?: {
        isHarmony(): boolean;
        isIOS(): boolean;
        isAndroid(): boolean;
        isMacOS(): boolean;
        isDesktop(): boolean;
        getOS(): string;
      };
      nativeComponents?: {
        send?: (msg: NativeComponentMessage) => void;
        register?: (id: string, handler: (msg: NativeComponentMessage) => void) => () => void;
      };
    };
  }
}
