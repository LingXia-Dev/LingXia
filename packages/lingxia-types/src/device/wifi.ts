/**
 * Wi-Fi APIs.
 */

export interface WifiInfo {
  SSID: string;
  BSSID?: string;
  secure: boolean;
  signalStrength: number;
  frequency?: number;
}

export interface WifiConnectedInfo extends WifiInfo {
  connected: boolean;
  state: string;
}

export interface ConnectWifiOptions {
  SSID: string;
  password?: string;
}

export type WifiConnectedCallback = (info: WifiConnectedInfo) => void;
