import { hilog } from '@kit.PerformanceAnalysisKit';
import { LxLog } from '../../app/LxLog';
import { BusinessError } from '@kit.BasicServicesKit';
import wifiManager from '@ohos.wifiManager';
import liblingxia from 'liblingxia.so';
import { PermissionManager } from '../../app/PermissionManager';

const DOMAIN = 0x0000;
const TAG = 'LingXia.Wifi';

export interface WifiInfo {
  ssid: string;
  bssid?: string;
  secure: boolean;
  signalStrength: number;
  frequency?: number;
  connected?: boolean;
  state?: string;
}

// Helper interface for accessing optional frequency property
interface WifiInfoWithFrequency {
  frequency?: number;
}

/**
 * WiFi management for HarmonyOS
 * Requires permissions:
 * - ohos.permission.GET_WIFI_INFO
 * - ohos.permission.SET_WIFI_INFO (for connect via addCandidateConfig)
 * - ohos.permission.LOCATION / ohos.permission.APPROXIMATELY_LOCATION (for scan)
 */
export class LxAppWifi {
  // Multi-LxApp support: maintain a set of state listeners
  private static stateCallbacks: Set<string> = new Set();
  private static wifiConnectionListener: ((state: number) => void) | null = null;
  private static lastConnectedSignature: string | null = null;
  private static emitInFlight: boolean = false;
  private static pendingEmit: boolean = false;
  private static pendingEmitHint: string = '';
  private static readonly SCAN_TIMEOUT_MS = 1000;

  // Active candidate network (HarmonyOS)
  private static activeCandidateNetworkId: number | null = null;
  private static targetSsid: string | null = null;  // Expected connected SSID

  /**
   * Initialize WiFi module
   * Note: WiFi permissions (ohos.permission.GET_WIFI_INFO) must be declared in module.json5
   */
  public static startWifi(callbackId: string): void {
    try {
      hilog.info(DOMAIN, TAG, 'WiFi module initialized');
      const success: boolean = liblingxia.onCallback(callbackId, true, '{}');
      if (!success) {
        LxLog.e(TAG, `Failed to send startWifi callback for ID: ${callbackId}`);
      }
    } catch (error) {
      const errorMessage = error instanceof Error ? error.message : String(error);
      LxLog.e(TAG, `startWifi error: ${errorMessage}`);
      liblingxia.onCallback(callbackId, false, '12001'); // System error
    }
  }

  /**
   * Stop WiFi module
   */
  public static stopWifi(callbackId: string): void {
    try {
      hilog.info(DOMAIN, TAG, 'WiFi module stopped');

      // Clean up active candidate network (HarmonyOS)
      if (LxAppWifi.activeCandidateNetworkId !== null) {
        try {
          wifiManager.removeCandidateConfig(LxAppWifi.activeCandidateNetworkId);
          hilog.info(DOMAIN, TAG, `Removed candidate network: ${LxAppWifi.activeCandidateNetworkId}`);
        } catch (error) {
          const errorMessage = error instanceof Error ? error.message : JSON.stringify(error);
          hilog.warn(DOMAIN, TAG, `Failed to remove candidate network: ${errorMessage}`);
        }
        LxAppWifi.activeCandidateNetworkId = null;
      }

      // Clear target SSID
      LxAppWifi.targetSsid = null;

      const success: boolean = liblingxia.onCallback(callbackId, true, '{}');
      if (!success) {
        LxLog.e(TAG, `Failed to send stopWifi callback for ID: ${callbackId}`);
      }
    } catch (error) {
      const errorMessage = error instanceof Error ? error.message : String(error);
      LxLog.e(TAG, `stopWifi error: ${errorMessage}`);
      liblingxia.onCallback(callbackId, false, '12001'); // System error
    }
  }

  /**
   * Add a listener for WiFi connection state changes
   * Multiple listeners can be registered (supports multiple LxApp instances)
   */
  public static addWifiStateListener(callbackId: string): void {
    hilog.info(DOMAIN, TAG, `addWifiStateListener: callbackId=${callbackId}`);

    if (LxAppWifi.stateCallbacks.add(callbackId)) {
      hilog.info(DOMAIN, TAG, `Added WiFi state listener: ${callbackId} (total=${LxAppWifi.stateCallbacks.size})`);

      // First subscriber: start system WiFi monitoring
      if (LxAppWifi.stateCallbacks.size === 1) {
        LxAppWifi.lastConnectedSignature = null;
        if (!LxAppWifi.wifiConnectionListener) {
          LxAppWifi.wifiConnectionListener = (state: number): void => {
            hilog.info(DOMAIN, TAG, `WiFi connection state changed: ${state}`);
            // Query current WiFi connection info when state changes and broadcast to all
            LxAppWifi.emitWifiConnectedToAll('');
          };
          try {
            wifiManager.on('wifiConnectionChange', LxAppWifi.wifiConnectionListener);
            hilog.info(DOMAIN, TAG, 'Registered system WiFi connection callback');
          } catch (error) {
            const errorMessage = error instanceof Error ? error.message : String(error);
            hilog.warn(DOMAIN, TAG, `wifiConnectionChange register failed: ${errorMessage}`);
          }
        }
      }

      // Send current state to new subscriber
      LxAppWifi.emitWifiConnectedToCallback(callbackId, '');
    } else {
      hilog.warn(DOMAIN, TAG, `WiFi state listener already exists: ${callbackId}`);
    }
  }

  /**
   * Remove a previously registered WiFi state listener
   */
  public static removeWifiStateListener(callbackId: string): void {
    hilog.info(DOMAIN, TAG, `removeWifiStateListener: callbackId=${callbackId}`);

    if (LxAppWifi.stateCallbacks.delete(callbackId)) {
      hilog.info(DOMAIN, TAG, `Removed WiFi state listener: ${callbackId} (remaining=${LxAppWifi.stateCallbacks.size})`);

      // Last subscriber: stop system WiFi monitoring
      if (LxAppWifi.stateCallbacks.size === 0) {
        if (LxAppWifi.wifiConnectionListener) {
          try {
            wifiManager.off('wifiConnectionChange', LxAppWifi.wifiConnectionListener);
            hilog.info(DOMAIN, TAG, 'Unregistered system WiFi connection callback');
          } catch (error) {
            const errorMessage = error instanceof Error ? error.message : String(error);
            hilog.warn(DOMAIN, TAG, `wifiConnectionChange unregister failed: ${errorMessage}`);
          }
        }
        LxAppWifi.wifiConnectionListener = null;
        LxAppWifi.lastConnectedSignature = null;
      }
    } else {
      hilog.warn(DOMAIN, TAG, `WiFi state listener not found: ${callbackId}`);
    }
  }

  /**
   * Connect to WiFi access point
   * Uses addCandidateConfig + connectToCandidateConfigWithUserAction for user-level apps.
   * Requires permission: ohos.permission.SET_WIFI_INFO
   */
  public static connectWifi(callbackId: string, ssid: string, password: string): void {
    (async () => {
      try {
        // Check WiFi permissions first
        const granted = await PermissionManager.ensureWifiAccess();
        if (!granted) {
          LxLog.e(TAG, 'WiFi permission not granted for connectWifi');
          liblingxia.onCallback(callbackId, false, '12006'); // Permission denied
          return;
        }

        // Remove any existing candidate network first
        if (LxAppWifi.activeCandidateNetworkId !== null) {
          try {
            wifiManager.removeCandidateConfig(LxAppWifi.activeCandidateNetworkId);
            hilog.info(DOMAIN, TAG, `Removed previous candidate network: ${LxAppWifi.activeCandidateNetworkId}`);
          } catch (error) {
            const errorMessage = error instanceof Error ? error.message : JSON.stringify(error);
            hilog.warn(DOMAIN, TAG, `Failed to remove previous candidate network: ${errorMessage}`);
          }
          LxAppWifi.activeCandidateNetworkId = null;
        }

        // Create WiFi device config
        const wifiDeviceConfig: wifiManager.WifiDeviceConfig = {
          ssid: ssid,
          preSharedKey: password || '',
          isHiddenSsid: false,
          securityType: password ? wifiManager.WifiSecurityType.WIFI_SEC_TYPE_PSK : wifiManager.WifiSecurityType.WIFI_SEC_TYPE_OPEN,
        };

        hilog.info(DOMAIN, TAG, `Connecting to WiFi: ${ssid}`);

        // Use addCandidateConfig + connectToCandidateConfigWithUserAction
        // This approach only requires SET_WIFI_INFO permission and prompts user for confirmation
        const networkId = await wifiManager.addCandidateConfig(wifiDeviceConfig);
        hilog.info(DOMAIN, TAG, `Added candidate config for ${ssid}, networkId: ${networkId}`);

        // Connect with user action - this will show a system dialog for user confirmation
        await wifiManager.connectToCandidateConfigWithUserAction(networkId);

        // Save the network ID and target SSID to maintain connection
        LxAppWifi.activeCandidateNetworkId = networkId;
        LxAppWifi.targetSsid = ssid;

        hilog.info(DOMAIN, TAG, `Successfully requested connection to WiFi: ${ssid}`);
        hilog.info(DOMAIN, TAG, `Actual connection state will be reported via wifiConnectionChange event`);

        const success: boolean = liblingxia.onCallback(callbackId, true, '{}');
        if (!success) {
          LxLog.e(TAG, `Failed to send connectWifi callback for ID: ${callbackId}`);
        }
        LxAppWifi.emitWifiConnectedToAll(ssid);
      } catch (error) {
        const businessError = error as BusinessError;
        const errorMessage = businessError.message || String(error);
        LxLog.e(TAG, `connectWifi error: ${errorMessage}, code: ${businessError.code}`);
        // Handle specific error codes
        if (businessError.code === 201) {
          liblingxia.onCallback(callbackId, false, '12006'); // Permission denied
        } else if (businessError.code === 2501005) {
          liblingxia.onCallback(callbackId, false, '12007'); // User did not respond
        } else if (businessError.code === 2501006) {
          liblingxia.onCallback(callbackId, false, '12008'); // User refused connection
        } else {
          liblingxia.onCallback(callbackId, false, '12002'); // Password error or invalid config
        }
      }
    })();
  }

  /**
   * Get WiFi list (scan results)
   */
  public static getWifiList(callbackId: string): void {
    (async () => {
      try {
        const granted = await PermissionManager.ensureWifiAccess();
        if (!granted) {
          LxLog.e(TAG, 'WiFi permission not granted for WiFi scan');
          liblingxia.onCallback(callbackId, false, '12006'); // Permission denied
          return;
        }

        // Use scan() for API 21+ (HarmonyOS Next)
        wifiManager.scan();

        // Wait for scan to complete, then get results
        setTimeout(() => {
          try {
            const scanInfos = wifiManager.getScanInfoList();
            const wifiList: WifiInfo[] = scanInfos.map((info: wifiManager.WifiScanInfo): WifiInfo => {
              // Convert signal strength from dBm to 0-100 scale
              // Typical range: -100 dBm (weak) to -30 dBm (strong)
              const rssi = info.rssi;
              let signalStrength = 0;
              if (rssi >= -30) {
                signalStrength = 100;
              } else if (rssi <= -100) {
                signalStrength = 0;
              } else {
                signalStrength = Math.floor(((rssi + 100) / 70) * 100);
              }

              const frequency = (info as WifiInfoWithFrequency).frequency;
              const wifiInfoItem: WifiInfo = {
                ssid: info.ssid,
                bssid: info.bssid,
                secure: info.securityType !== wifiManager.WifiSecurityType.WIFI_SEC_TYPE_OPEN,
                signalStrength: Math.min(100, Math.max(0, signalStrength)),
                frequency: typeof frequency === 'number' ? frequency : undefined,
              };
              return wifiInfoItem;
            });

            hilog.info(DOMAIN, TAG, `Found ${wifiList.length} WiFi networks`);
            const success: boolean = liblingxia.onCallback(callbackId, true, JSON.stringify(wifiList));
            if (!success) {
              LxLog.e(TAG, `Failed to send getWifiList callback for ID: ${callbackId}`);
            }
          } catch (resultError) {
            const errorMessage = resultError instanceof Error ? resultError.message : JSON.stringify(resultError);
            LxLog.e(TAG, `Failed to get scan results: ${errorMessage}`);
            liblingxia.onCallback(callbackId, false, '12001'); // System error
          }
        }, LxAppWifi.SCAN_TIMEOUT_MS);
      } catch (error) {
        const errorMessage = error instanceof Error ? error.message : JSON.stringify(error);
        LxLog.e(TAG, `getWifiList error: ${errorMessage}`);
        liblingxia.onCallback(callbackId, false, '12001'); // System error
      }
    })();
  }

  /**
   * Get connected WiFi info
   */
  public static getConnectedWifi(callbackId: string): void {
    (async () => {
      try {
        const granted = await PermissionManager.ensureWifiAccess();
        if (!granted) {
          LxLog.e(TAG, 'WiFi permission not granted for getConnectedWifi');
          liblingxia.onCallback(callbackId, false, '12006'); // Permission denied
          return;
        }

        // Get linked (connected) WiFi info
        wifiManager.getLinkedInfo((error: Error, linkedInfo: wifiManager.WifiLinkedInfo): void => {
          if (error) {
            LxLog.e(TAG, `Failed to get linked info: ${error.message}`);
            liblingxia.onCallback(callbackId, false, '12001'); // System error
            return;
          }

          if (!linkedInfo || linkedInfo.ssid === '') {
            hilog.warn(DOMAIN, TAG, 'No WiFi connected');
            liblingxia.onCallback(callbackId, false, '12001'); // System error (not connected)
            return;
          }

          // Convert RSSI to signal strength (0-100)
          const rssi = linkedInfo.rssi;
          let signalStrength = 0;
          if (rssi >= -30) {
            signalStrength = 100;
          } else if (rssi <= -100) {
            signalStrength = 0;
          } else {
            signalStrength = Math.floor(((rssi + 100) / 70) * 100);
          }

          const frequency = (linkedInfo as WifiInfoWithFrequency).frequency;
          const wifiInfo: WifiInfo = {
            ssid: linkedInfo.ssid,
            bssid: linkedInfo.bssid,
            secure: true, // Assume secure if connected
            signalStrength: Math.min(100, Math.max(0, signalStrength)),
            frequency: typeof frequency === 'number' ? frequency : undefined,
          };

          hilog.info(DOMAIN, TAG, `Connected to WiFi: ${linkedInfo.ssid}`);
          const success: boolean = liblingxia.onCallback(callbackId, true, JSON.stringify(wifiInfo));
          if (!success) {
            LxLog.e(TAG, `Failed to send getConnectedWifi callback for ID: ${callbackId}`);
          }
        });
      } catch (error) {
        const errorMessage = error instanceof Error ? error.message : String(error);
        LxLog.e(TAG, `getConnectedWifi error: ${errorMessage}`);
        liblingxia.onCallback(callbackId, false, '12001'); // System error
      }
    })();
  }

  /**
   * Build WiFi info JSON with deduplication
   */
  private static async buildWifiInfoJson(ssidHint: string): Promise<WifiInfo | null> {
    try {
      const linkedInfo: wifiManager.WifiLinkedInfo = await wifiManager.getLinkedInfo();
      const linkedSsid = linkedInfo && linkedInfo.ssid ? linkedInfo.ssid : '';
      const ssid = linkedSsid || ssidHint || '';
      const connected = Boolean(linkedSsid || ssidHint);
      const bssid = connected ? linkedInfo?.bssid : undefined;
      const frequency = connected ? (linkedInfo as WifiInfoWithFrequency)?.frequency : undefined;

      let signalStrength = 0;
      if (connected && linkedInfo) {
        const rssi = linkedInfo.rssi;
        if (rssi >= -30) {
          signalStrength = 100;
        } else if (rssi <= -100) {
          signalStrength = 0;
        } else {
          signalStrength = Math.floor(((rssi + 100) / 70) * 100);
        }
        signalStrength = Math.min(100, Math.max(0, signalStrength));
      }

      const signature = `${connected ? '1' : '0'}|${ssid}|${bssid ?? ''}|${frequency ?? ''}`;

      // Deduplicate events by signature
      if (signature === LxAppWifi.lastConnectedSignature) {
        return null;
      }
      LxAppWifi.lastConnectedSignature = signature;

      // Skip initial disconnected state
      if (!connected && ssid === '' && signature === '0|||') {
        return null;
      }

      // Detect unexpected network switch (HarmonyOS auto-switch issue)
      if (LxAppWifi.targetSsid && connected && linkedSsid && linkedSsid !== LxAppWifi.targetSsid) {
        hilog.warn(DOMAIN, TAG,
          `⚠️  WiFi switched from target "${LxAppWifi.targetSsid}" to "${linkedSsid}". ` +
          `HarmonyOS candidate config API does not maintain persistent connections. ` +
          `System auto-switched based on signal strength/network preference.`);
      }

      const wifiInfo: WifiInfo = {
        ssid: ssid,
        bssid: bssid,
        secure: connected ? true : false,
        signalStrength: signalStrength,
        frequency: typeof frequency === 'number' ? frequency : undefined,
        connected: connected,
        state: connected ? 'connected' : 'disconnected',
      };

      return wifiInfo;
    } catch (error) {
      const errorMessage = error instanceof Error ? error.message : JSON.stringify(error);
      LxLog.e(TAG, `buildWifiInfoJson error: ${errorMessage}`);
      return null;
    }
  }

  /**
   * Emit WiFi connected event to a specific callback
   */
  private static emitWifiConnectedToCallback(callbackId: string, ssidHint: string): void {
    LxAppWifi.buildWifiInfoJson(ssidHint).then((wifiInfo: WifiInfo | null) => {
      if (!wifiInfo) {
        return;
      }

      hilog.info(DOMAIN, TAG, `emitWifiConnected: callbackId=${callbackId}`);
      const success: boolean = liblingxia.onCallback(callbackId, true, JSON.stringify(wifiInfo));
      if (!success) {
        hilog.warn(DOMAIN, TAG, `Failed to dispatch wifi connected event to callback ${callbackId}`);
      }
    }).catch((err: Error) => {
      LxLog.e(TAG, `emitWifiConnectedToCallback error: ${err.message}`);
    });
  }

  /**
   * Broadcast WiFi connected event to all subscribers
   */
  private static emitWifiConnectedToAll(ssidHint: string): void {
    if (LxAppWifi.stateCallbacks.size === 0) {
      return;
    }

    if (LxAppWifi.emitInFlight) {
      LxAppWifi.pendingEmit = true;
      if (ssidHint) {
        LxAppWifi.pendingEmitHint = ssidHint;
      }
      return;
    }

    LxAppWifi.emitInFlight = true;
    LxAppWifi.buildWifiInfoJson(ssidHint).then((wifiInfo: WifiInfo | null) => {
      if (!wifiInfo) {
        return;
      }

      hilog.info(DOMAIN, TAG, `emitWifiConnectedToAll: ${LxAppWifi.stateCallbacks.size} subscribers`);
      const jsonString = JSON.stringify(wifiInfo);
      for (const callbackId of LxAppWifi.stateCallbacks) {
        const success: boolean = liblingxia.onCallback(callbackId, true, jsonString);
        if (!success) {
          hilog.warn(DOMAIN, TAG, `Failed to dispatch wifi connected event to callback ${callbackId}`);
        }
      }
    }).catch((err: Error) => {
      LxLog.e(TAG, `emitWifiConnectedToAll error: ${err.message}`);
    }).finally(() => {
      LxAppWifi.emitInFlight = false;
      if (LxAppWifi.pendingEmit) {
        const hint = LxAppWifi.pendingEmitHint;
        LxAppWifi.pendingEmit = false;
        LxAppWifi.pendingEmitHint = '';
        LxAppWifi.emitWifiConnectedToAll(hint);
      }
    });
  }
}
