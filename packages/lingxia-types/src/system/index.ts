/**
 * System settings and URL APIs.
 */

export interface SystemSettingInfo {
  bluetoothEnabled: boolean;
  locationEnabled: boolean;
  wifiEnabled: boolean;
}

export interface OpenURLOptions {
  url: string;
  target?: 'self' | 'external';
}
