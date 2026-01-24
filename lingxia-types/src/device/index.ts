/**
 * Device APIs
 * Corresponds to: lingxia-logic/src/device/
 */

export interface DeviceInfo {
  brand: string;
  model: string;
  marketName: string;
  system: string;
}

export interface ScreenInfo {
  width: number;
  height: number;
  scale: number;
}

export interface MakePhoneCallOptions {
  phoneNumber: string;
}

export interface WifiInfo {
  SSID: string;
  BSSID?: string;
  secure: boolean;
  signalStrength: number;
  frequency?: number;
}

export interface ConnectWifiOptions {
  SSID: string;
  password?: string;
}

export type WifiConnectedCallback = (info: WifiInfo) => void;

export interface AppOrientationInfo {
  orientation: string;
}

export interface SetAppOrientationOptions {
  orientation: string;
}

export interface KeyEvent {
  key: string;
  code: string;
  altKey?: boolean;
  ctrlKey?: boolean;
  shiftKey?: boolean;
  metaKey?: boolean;
}

export type KeyEventCallback = (event: KeyEvent) => void;
