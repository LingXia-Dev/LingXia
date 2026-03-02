/**
 * System APIs
 * Corresponds to: lingxia-logic/src/system.rs
 */

export interface AppBaseInfo {
  language: string;
  productName: string;
  version: string;
  SDKVersion: string;
}

export interface SystemSettingInfo {
  bluetoothEnabled: boolean;
  locationEnabled: boolean;
  wifiEnabled: boolean;
}

export interface OpenURLOptions {
  url: string;
  openIn?: 'external' | 'internal';
}
