import type { NativeComponentMessage as BridgeNativeComponentMessage } from '@lingxia/bridge';

// Window.LingXiaBridge is declared by @lingxia/bridge (LingXiaBridgeInterface).
// Do not add a conflicting Window.LingXiaBridge declaration here.

/**
 * Elements-level NativeComponentMessage — extends bridge's open type with known fields.
 * This richer type is used internally and exported for component implementors.
 */
export interface NativeComponentMessage extends BridgeNativeComponentMessage {
  action: string;
  type?: string;
  event?: string;
  detail?: unknown;
  payload?: unknown;
  rect?: { x: number; y: number; width: number; height: number };
  props?: Record<string, unknown>;
  zIndex?: number;
  cornerRadius?: number;
}
