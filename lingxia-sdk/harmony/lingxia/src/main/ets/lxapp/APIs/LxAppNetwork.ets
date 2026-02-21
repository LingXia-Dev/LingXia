import { hilog } from '@kit.PerformanceAnalysisKit';
import connection from '@ohos.net.connection';
import radio from '@ohos.telephony.radio';
import liblingxia from 'liblingxia.so';

const DOMAIN = 0x0000;
const TAG = 'LingXia.Network';

interface NetworkInfo {
  isConnected: boolean;
  networkType: string;
  ipv4: string[];
  ipv6: string[];
}

interface CurrentNetworkStatus {
  isConnected: boolean;
  networkType: string;
}

interface LocalIPAddresses {
  ipv4: string[];
  ipv6: string[];
}

export class LxAppNetwork {
  private static changeCallbacks: Set<string> = new Set();
  private static netConnection: connection.NetConnection | null = null;
  private static registerCallback: (() => void) | null = null;
  private static lastSignature: string = '';

  public static getNetworkInfo(callbackId: string): void {
    LxAppNetwork.queryNetworkInfo().then((info: NetworkInfo) => {
      liblingxia.onCallback(callbackId, true, JSON.stringify(info));
    }).catch((error: Error) => {
      const message = error instanceof Error ? error.message : `${error}`;
      hilog.warn(DOMAIN, TAG, `getNetworkInfo failed: ${message}`);
      liblingxia.onCallback(callbackId, false, '1000');
    });
  }

  public static addNetworkChangeListener(callbackId: string): void {
    LxAppNetwork.changeCallbacks.add(callbackId);
    if (LxAppNetwork.changeCallbacks.size === 1) {
      LxAppNetwork.startMonitor();
    }
    LxAppNetwork.emitToCallback(callbackId);
  }

  public static removeNetworkChangeListener(callbackId: string): void {
    LxAppNetwork.changeCallbacks.delete(callbackId);
    if (LxAppNetwork.changeCallbacks.size === 0) {
      LxAppNetwork.stopMonitor();
      LxAppNetwork.lastSignature = '';
    }
  }

  private static startMonitor(): void {
    if (LxAppNetwork.netConnection) {
      return;
    }
    try {
      const monitor = connection.createNetConnection();
      monitor.on('netAvailable', () => { LxAppNetwork.emitToAll(); });
      monitor.on('netCapabilitiesChange', () => { LxAppNetwork.emitToAll(); });
      monitor.on('netLost', () => { LxAppNetwork.emitToAll(); });
      monitor.on('netUnavailable', () => { LxAppNetwork.emitToAll(); });

      const cb = (): void => {
        // no-op callback; status comes from events
      };
      monitor.register(cb);
      LxAppNetwork.registerCallback = cb;
      LxAppNetwork.netConnection = monitor;
    } catch (error) {
      const message = error instanceof Error ? error.message : `${error}`;
      hilog.warn(DOMAIN, TAG, `startMonitor failed: ${message}`);
    }
  }

  private static stopMonitor(): void {
    const monitor = LxAppNetwork.netConnection;
    if (!monitor) {
      return;
    }
    try {
      const cb = LxAppNetwork.registerCallback;
      if (cb) {
        monitor.unregister(cb);
      }
    } catch (error) {
      const message = error instanceof Error ? error.message : `${error}`;
      hilog.warn(DOMAIN, TAG, `stopMonitor failed: ${message}`);
    } finally {
      LxAppNetwork.netConnection = null;
      LxAppNetwork.registerCallback = null;
    }
  }

  private static emitToCallback(callbackId: string): void {
    LxAppNetwork.queryNetworkInfo().then((info: NetworkInfo) => {
      liblingxia.onCallback(callbackId, true, JSON.stringify(info));
    }).catch(() => {
      const fallback: NetworkInfo = {
        isConnected: false,
        networkType: 'none',
        ipv4: [],
        ipv6: [],
      };
      liblingxia.onCallback(callbackId, true, JSON.stringify(fallback));
    });
  }

  private static emitToAll(): void {
    if (LxAppNetwork.changeCallbacks.size === 0) {
      return;
    }
    LxAppNetwork.queryNetworkInfo().then((info: NetworkInfo) => {
      const signature = LxAppNetwork.buildSignature(info);
      if (signature === LxAppNetwork.lastSignature) {
        return;
      }
      LxAppNetwork.lastSignature = signature;
      const payload = JSON.stringify(info);
      LxAppNetwork.changeCallbacks.forEach((callbackId: string) => {
        liblingxia.onCallback(callbackId, true, payload);
      });
    }).catch((error: Error) => {
      hilog.warn(DOMAIN, TAG, `emitToAll failed: ${error.message}`);
    });
  }

  private static buildSignature(info: NetworkInfo): string {
    return `${info.isConnected}:${info.networkType}:${info.ipv4.join(',')}:${info.ipv6.join(',')}`;
  }

  private static async queryCurrentStatus(): Promise<CurrentNetworkStatus> {
    try {
      const hasDefault = await connection.hasDefaultNet();
      if (!hasDefault) {
        return { isConnected: false, networkType: 'none' };
      }

      const netHandle = await connection.getDefaultNet();
      if (!netHandle) {
        return { isConnected: false, networkType: 'none' };
      }

      const netCaps = await connection.getNetCapabilities(netHandle);
      const rawBearerTypes = netCaps?.bearerTypes ?? [];
      const bearerTypes = Array.isArray(rawBearerTypes) ? rawBearerTypes.map((value) => Number(value)) : [];
      if (bearerTypes.length === 0) {
        return { isConnected: true, networkType: 'unknown' };
      }

      const WIFI = Number(connection.NetBearType.BEARER_WIFI);
      const CELLULAR = Number(connection.NetBearType.BEARER_CELLULAR);
      const ETHERNET = Number(connection.NetBearType.BEARER_ETHERNET);
      if (bearerTypes.includes(WIFI)) {
        return { isConnected: true, networkType: 'wifi' };
      }
      if (bearerTypes.includes(CELLULAR)) {
        const networkType = await LxAppNetwork.queryCellularNetworkType();
        return { isConnected: true, networkType };
      }
      if (bearerTypes.includes(ETHERNET)) {
        return { isConnected: true, networkType: 'ethernet' };
      }
      return { isConnected: true, networkType: 'unknown' };
    } catch (error) {
      const message = error instanceof Error ? error.message : `${error}`;
      hilog.warn(DOMAIN, TAG, `queryCurrentStatus failed: ${message}`);
      return { isConnected: false, networkType: 'none' };
    }
  }

  private static async queryNetworkInfo(): Promise<NetworkInfo> {
    const status = await LxAppNetwork.queryCurrentStatus();
    if (!status.isConnected) {
      return { isConnected: false, networkType: 'none', ipv4: [], ipv6: [] };
    }

    const ips = await LxAppNetwork.queryLocalIPAddresses();
    return {
      isConnected: true,
      networkType: status.networkType,
      ipv4: ips.ipv4,
      ipv6: ips.ipv6,
    };
  }

  private static mapRadioTechToNetworkType(rat: number | undefined | null): string {
    switch (rat) {
      case radio.RadioTechnology.RADIO_TECHNOLOGY_GSM:
      case radio.RadioTechnology.RADIO_TECHNOLOGY_1XRTT:
        return '2g';
      case radio.RadioTechnology.RADIO_TECHNOLOGY_WCDMA:
      case radio.RadioTechnology.RADIO_TECHNOLOGY_HSPA:
      case radio.RadioTechnology.RADIO_TECHNOLOGY_HSPAP:
      case radio.RadioTechnology.RADIO_TECHNOLOGY_TD_SCDMA:
      case radio.RadioTechnology.RADIO_TECHNOLOGY_EVDO:
      case radio.RadioTechnology.RADIO_TECHNOLOGY_EHRPD:
        return '3g';
      case radio.RadioTechnology.RADIO_TECHNOLOGY_LTE:
      case radio.RadioTechnology.RADIO_TECHNOLOGY_LTE_CA:
        return '4g';
      case radio.RadioTechnology.RADIO_TECHNOLOGY_NR:
        return '5g';
      case radio.RadioTechnology.RADIO_TECHNOLOGY_IWLAN:
        // IWLAN is typically WiFi calling; map to wifi rather than a cellular generation.
        return 'wifi';
      case radio.RadioTechnology.RADIO_TECHNOLOGY_UNKNOWN:
      default:
        return 'unknown';
    }
  }

  private static async queryCellularNetworkType(): Promise<string> {
    // `@ohos.telephony.radio` typing varies by SDK version; we only need `psRadioTech`.
    interface RadioTechResult {
      psRadioTech?: number;
    }
    try {
      const slotId = await radio.getPrimarySlotId().catch((): number => 0);
      const tech = (await radio.getRadioTech(slotId)) as RadioTechResult;
      const psRadioTech = typeof tech.psRadioTech === 'number'
        ? tech.psRadioTech
        : radio.RadioTechnology.RADIO_TECHNOLOGY_UNKNOWN;
      const psRat = Number(psRadioTech);
      const mapped = LxAppNetwork.mapRadioTechToNetworkType(psRat);
      // If we are on a cellular bearer but cannot classify the generation, keep it as `cellular`.
      return mapped === 'unknown' ? 'cellular' : mapped;
    } catch (error) {
      const message = error instanceof Error ? error.message : `${error}`;
      hilog.warn(DOMAIN, TAG, `queryCellularNetworkType failed: ${message}`);
      return 'cellular';
    }
  }

  private static async queryLocalIPAddresses(): Promise<LocalIPAddresses> {
    const ipv4Seen: Set<string> = new Set<string>();
    const ipv6Seen: Set<string> = new Set<string>();
    try {
      const hasDefault = await connection.hasDefaultNet();
      if (!hasDefault) {
        return { ipv4: [], ipv6: [] };
      }

      const netHandle = await connection.getDefaultNet();
      if (!netHandle) {
        return { ipv4: [], ipv6: [] };
      }
      const properties = await connection.getConnectionProperties(netHandle);
      const links = properties?.linkAddresses ?? [];
      for (let i = 0; i < links.length; i++) {
        const netAddress = links[i]?.address;
        const family = Number(netAddress?.family ?? 0);
        const rawValue = String(netAddress?.address ?? '').trim();
        const valueWithPrefix = rawValue.split('/')[0].trim();
        const value = valueWithPrefix.split('%')[0].trim();
        if (value.length === 0) {
          continue;
        }

        const looksIPv4 = /^\d{1,3}(\.\d{1,3}){3}$/.test(value);
        const looksIPv6 = value.includes(':');
        const isIPv4 = family === 1 || looksIPv4;
        const isIPv6 = family === 2 || looksIPv6;

        if (isIPv4) {
          const isLoopbackV4 = value.startsWith('127.');
          const isZeroV4 = value === '0.0.0.0';
          if (!isLoopbackV4 && !isZeroV4) {
            ipv4Seen.add(value);
          }
          continue;
        }

        if (isIPv6) {
          const lower = value.toLowerCase();
          const isLoopbackV6 = lower === '::1';
          const isZeroV6 = lower === '::';
          const isLinkLocalV6 = lower.startsWith('fe80:');
          if (!isLoopbackV6 && !isZeroV6 && !isLinkLocalV6) {
            ipv6Seen.add(value);
          }
        }
      }
    } catch (error) {
      const message = error instanceof Error ? error.message : `${error}`;
      hilog.warn(DOMAIN, TAG, `queryLocalIPAddresses failed: ${message}`);
    }
    const ipv4 = Array.from(ipv4Seen).sort();
    const ipv6 = Array.from(ipv6Seen).sort();
    return {
      ipv4: ipv4.length > 0 ? [ipv4[0]] : [],
      ipv6: ipv6.length > 0 ? [ipv6[0]] : [],
    };
  }
}
