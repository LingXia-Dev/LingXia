/**
 * System APIs
 * Corresponds to: lingxia-logic/src/system.rs, navigator.rs, update.rs
 */

export interface AppBaseInfo {
  language: string;
  productName: string;
  version: string;
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

export interface NavigateToLxAppOptions {
  appId: string;
  path?: string;
  envVersion?: 'develop' | 'trial' | 'release';
}

export interface UpdateManager {
  applyUpdate(): void;
  onUpdateReady(callback: () => void): void;
  onUpdateFailed(callback: () => void): void;
}
