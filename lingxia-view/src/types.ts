/**
 * Shared type definitions for LingXia View.
 */

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
      platform?: {
        isHarmony(): boolean;
        isIOS(): boolean;
        isAndroid(): boolean;
        getOS(): string;
      };
      sameLevel?: {
        send?: (msg: SameLevelMessage) => void;
        register?: (id: string, handler: (msg: SameLevelMessage) => void) => () => void;
      };
    };
  }
}
