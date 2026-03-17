/**
 * Device info APIs
 * Corresponds to: lingxia-logic/src/device/info.rs
 */

export interface DeviceInfo {
  brand: string;
  model: string;
  marketName: string;
  osName: string;
  osVersion: string;
}

export interface ScreenInfo {
  width: number;
  height: number;
  scale: number;
}
