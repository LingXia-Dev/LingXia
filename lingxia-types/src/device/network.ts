/**
 * Network APIs
 * Corresponds to: lingxia-logic/src/device/network.rs
 */

// Matches mini-program style where possible: wifi/2g/3g/4g/5g/none/unknown.
export type NetworkType =
  | 'none'
  | 'unknown'
  | 'wifi'
  | '2g'
  | '3g'
  | '4g'
  | '5g'
  | 'ethernet';

export interface NetworkInfo {
  isConnected: boolean;
  networkType: NetworkType;
  ipv4: string[];
  ipv6: string[];
}

export type NetworkChangeCallback = (info: NetworkInfo) => void;
