import { hilog } from '@kit.PerformanceAnalysisKit';
import { abilityAccessCtrl, Permissions } from '@kit.AbilityKit';
import common from '@ohos.app.ability.common';
import { onCallback } from 'liblingxia.so';
import { LxLog } from './LxLog';

const DOMAIN = 0x0000;
const TAG = 'LingXia.Permission';

// abilityInfo in Harmony UIAbilityContext can expose a tokenId used for
// permission checks via AbilityAccessCtrl.
interface AbilityInfoWithTokenId {
  tokenId?: number;
}

// Harmony PermissionResult code for PERMISSION_GRANTED.
function permissionGrantedCode(): number {
  return 0;
}

export class PermissionManager {
  private static context: common.UIAbilityContext | null = null;
  private static granted: Set<Permissions> = new Set();

  public static initialize(context: common.UIAbilityContext): void {
    PermissionManager.context = context;
  }

  public static hasContext(): boolean {
    return PermissionManager.context !== null;
  }

  public static async ensureCameraAccess(includeMicrophone: boolean): Promise<boolean> {
    const required: Permissions[] = ['ohos.permission.CAMERA' as Permissions];
    if (includeMicrophone) {
      required.push('ohos.permission.MICROPHONE' as Permissions);
    }
    const granted = await PermissionManager.ensurePermissions(required);
    if (!granted) {
      hilog.warn(DOMAIN, TAG, 'Camera/microphone permission not granted');
    }
    return granted;
  }

  public static async ensureMicrophoneAccess(): Promise<boolean> {
    const granted = await PermissionManager.ensurePermissions([
      'ohos.permission.MICROPHONE' as Permissions
    ]);
    if (!granted) {
      hilog.warn(DOMAIN, TAG, 'Microphone permission not granted');
    }
    return granted;
  }

  public static async ensureMediaWriteAccess(): Promise<boolean> {
    const granted = await PermissionManager.ensurePermissions([
      'ohos.permission.WRITE_IMAGEVIDEO' as Permissions
    ]);
    if (!granted) {
      hilog.warn(DOMAIN, TAG, 'Media write permission not granted');
    }
    return granted;
  }

  public static async ensureLocationAccess(): Promise<boolean> {
    const permissions: Permissions[] = [
      'ohos.permission.LOCATION' as Permissions,
      'ohos.permission.APPROXIMATELY_LOCATION' as Permissions
    ];
    const ctx = PermissionManager.context;
    if (!ctx) {
      hilog.warn(DOMAIN, TAG, 'ensureLocationAccess called before context is ready');
      return false;
    }

    if (await PermissionManager.hasAnyPermission(ctx, permissions)) {
      return true;
    }

    const grantedAll = await PermissionManager.ensurePermissions(permissions);
    if (grantedAll) {
      return true;
    }

    const grantedAny = await PermissionManager.hasAnyPermission(ctx, permissions);
    if (!grantedAny) {
      hilog.warn(DOMAIN, TAG, 'Location permission not granted');
    }
    return grantedAny;
  }

  public static requestLocationPermission(callbackId: string): boolean {
    const id = String(callbackId ?? '').trim();
    if (!id) {
      hilog.warn(DOMAIN, TAG, 'requestLocationPermission: missing callbackId');
      return false;
    }

    (async () => {
      try {
        const granted: boolean = await PermissionManager.ensureLocationAccess();
        if (granted) {
          onCallback(id, true, '{}');
        } else {
          onCallback(id, false, '3002');
        }
        hilog.info(DOMAIN, TAG, `requestLocationPermission result: ${granted}`);
      } catch (error) {
        const message = String(error);
        LxLog.e(TAG, `requestLocationPermission failed: ${message}`);
        onCallback(id, false, '1000');
      }
    })();

    return true;
  }

  public static async ensureWifiAccess(): Promise<boolean> {
    const ctx = PermissionManager.context;
    if (!ctx) {
      hilog.warn(DOMAIN, TAG, 'ensureWifiAccess called before context is ready');
      return false;
    }

    // Request both GET and SET WiFi permissions
    const wifiPermissionsGranted = await PermissionManager.ensurePermissions([
      'ohos.permission.GET_WIFI_INFO' as Permissions,
      'ohos.permission.SET_WIFI_INFO' as Permissions
    ]);
    if (!wifiPermissionsGranted) {
      hilog.warn(DOMAIN, TAG, 'WiFi permissions not granted');
      return false;
    }

    const locationGranted = await PermissionManager.ensureLocationAccess();
    if (!locationGranted) {
      hilog.warn(DOMAIN, TAG, 'WiFi location permission not granted');
      return false;
    }
    return true;
  }

  private static async hasAnyPermission(
    context: common.UIAbilityContext,
    permissions: Permissions[]
  ): Promise<boolean> {
    const missing = await PermissionManager.collectMissingPermissions(context, permissions);
    return missing.length < permissions.length;
  }

  public static async ensurePermissions(permissions: Permissions[]): Promise<boolean> {
    if (!permissions || permissions.length === 0) {
      return true;
    }
    const ctx = PermissionManager.context;
    if (!ctx) {
      hilog.warn(DOMAIN, TAG, 'ensurePermissions called before context is ready');
      return false;
    }

    const missing = await PermissionManager.collectMissingPermissions(ctx, permissions);
    if (missing.length === 0) {
      return true;
    }

    return PermissionManager.requestPermissions(ctx, missing);
  }

  private static async collectMissingPermissions(
    _context: common.UIAbilityContext,
    permissions: Permissions[]
  ): Promise<Permissions[]> {
    const atManager = abilityAccessCtrl.createAtManager();

    const ctx = PermissionManager.context;
    let tokenId: number | undefined;
    if (ctx && (ctx as common.UIAbilityContext).abilityInfo) {
      const abilityInfo = (ctx as common.UIAbilityContext).abilityInfo as AbilityInfoWithTokenId;
      tokenId = abilityInfo.tokenId;
    }

    const grantedCode = permissionGrantedCode();
    const missing: Permissions[] = [];

    for (const permission of permissions) {
      if (PermissionManager.granted.has(permission)) {
        continue;
      }

      let granted = false;
      if (typeof tokenId === 'number') {
        try {
          const status = await atManager.checkAccessToken(tokenId, permission);
          granted = (status === grantedCode);
        } catch (error) {
          hilog.warn(
            DOMAIN,
            TAG,
            `checkAccessToken failed for ${permission}: ${String(error)}`
          );
        }
      }

      if (granted) {
        PermissionManager.granted.add(permission);
      } else {
        missing.push(permission);
      }
    }

    return missing;
  }

  private static async requestPermissions(
    context: common.UIAbilityContext,
    permissions: Permissions[]
  ): Promise<boolean> {
    try {
      const atManager = abilityAccessCtrl.createAtManager();
      const result = await atManager.requestPermissionsFromUser(context, permissions);
      const grantedCode = permissionGrantedCode();
      let allGranted = true;

      if (result && Array.isArray(result.authResults)) {
        result.authResults.forEach((status: number, index: number) => {
          const permission = permissions[index];
          if (status === grantedCode) {
            PermissionManager.granted.add(permission);
          } else {
            allGranted = false;
          }
        });
      } else {
        allGranted = false;
      }

      return allGranted;
    } catch (error) {
      LxLog.e(TAG, `requestPermissionsFromUser failed: ${String(error)}`);
      return false;
    }
  }
}
