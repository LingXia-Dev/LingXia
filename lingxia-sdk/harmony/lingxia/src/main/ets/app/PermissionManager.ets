import { hilog } from '@kit.PerformanceAnalysisKit';
import { abilityAccessCtrl, Permissions } from '@kit.AbilityKit';
import common from '@ohos.app.ability.common';
import { onCallback } from 'liblingxia.so';
import { LxLog } from './LxLog';
import { PermissionFeature, PermissionGates, PermissionOutcome } from './PermissionGate';

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

  public static async ensureCameraAccess(feature: PermissionFeature, includeMicrophone: boolean): Promise<boolean> {
    const required: Permissions[] = ['ohos.permission.CAMERA' as Permissions];
    if (includeMicrophone) {
      required.push('ohos.permission.MICROPHONE' as Permissions);
    }
    const granted = await PermissionManager.ensurePermissions(required, feature);
    if (!granted) {
      hilog.warn(DOMAIN, TAG, 'Camera/microphone permission not granted');
    }
    return granted;
  }

  public static async ensureMediaWriteAccess(): Promise<boolean> {
    const granted = await PermissionManager.ensurePermissions([
      'ohos.permission.WRITE_IMAGEVIDEO' as Permissions
    ], PermissionFeature.SAVE_MEDIA);
    if (!granted) {
      hilog.warn(DOMAIN, TAG, 'Media write permission not granted');
    }
    return granted;
  }

  public static async ensureLocationAccess(
    feature: PermissionFeature = PermissionFeature.LOCATION
  ): Promise<boolean> {
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

    const grantedAll = await PermissionManager.ensurePermissions(permissions, feature, true);
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
    ], PermissionFeature.WIFI);
    if (!wifiPermissionsGranted) {
      hilog.warn(DOMAIN, TAG, 'WiFi permissions not granted');
      return false;
    }

    const locationGranted = await PermissionManager.ensureLocationAccess(PermissionFeature.WIFI);
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

  /** [anyIsEnough]: one granted permission satisfies the request (precise vs approximate location). */
  public static async ensurePermissions(
    permissions: Permissions[],
    feature: PermissionFeature,
    anyIsEnough: boolean = false
  ): Promise<boolean> {
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

    const blocked = missing.some((permission: Permissions) => PermissionGates.asked(ctx, permission));
    return PermissionGates.run(ctx, feature, missing, blocked, () => {
      PermissionGates.markAsked(ctx, missing);
      // Harmony never re-shows a denied prompt; with a gate deciding to go
      // ahead, offer the system re-grant sheet instead of failing silently.
      return blocked && PermissionGates.isGated()
        ? PermissionManager.requestOnSetting(ctx, missing, anyIsEnough)
        : PermissionManager.requestPermissions(ctx, missing, anyIsEnough);
    });
  }

  public static async isGranted(permission: Permissions): Promise<boolean> {
    const ctx = PermissionManager.context;
    if (!ctx) {
      return false;
    }
    return (await PermissionManager.collectMissingPermissions(ctx, [permission])).length === 0;
  }

  private static outcomeOf(results: number[], count: number, anyIsEnough: boolean): PermissionOutcome {
    const grantedCode = permissionGrantedCode();
    const grantedCount = results.filter((code: number) => code === grantedCode).length;
    if (results.length === count && (anyIsEnough ? grantedCount > 0 : grantedCount === count)) {
      return PermissionOutcome.GRANTED;
    }
    // -1 is a user denial, which Harmony never prompts for again.
    return results.some((code: number) => code === -1)
      ? PermissionOutcome.DENIED_PERMANENTLY
      : PermissionOutcome.DENIED;
  }

  private static async requestOnSetting(
    context: common.UIAbilityContext,
    permissions: Permissions[],
    anyIsEnough: boolean
  ): Promise<PermissionOutcome> {
    try {
      const atManager = abilityAccessCtrl.createAtManager();
      const results = await atManager.requestPermissionOnSetting(context, permissions);
      const outcome = PermissionManager.outcomeOf(results as number[], permissions.length, anyIsEnough);
      if (outcome === PermissionOutcome.GRANTED) {
        permissions.forEach((permission: Permissions) => PermissionManager.granted.add(permission));
      }
      return outcome === PermissionOutcome.GRANTED ? outcome : PermissionOutcome.DENIED_PERMANENTLY;
    } catch (error) {
      LxLog.e(TAG, `requestPermissionOnSetting failed: ${String(error)}`);
      return PermissionOutcome.DENIED_PERMANENTLY;
    }
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
    permissions: Permissions[],
    anyIsEnough: boolean
  ): Promise<PermissionOutcome> {
    try {
      const atManager = abilityAccessCtrl.createAtManager();
      const result = await atManager.requestPermissionsFromUser(context, permissions);
      if (!result || !Array.isArray(result.authResults)) {
        return PermissionOutcome.DENIED;
      }
      const grantedCode = permissionGrantedCode();
      permissions.forEach((permission: Permissions, index: number) => {
        if (result.authResults[index] === grantedCode) {
          PermissionManager.granted.add(permission);
        }
      });
      return PermissionManager.outcomeOf(result.authResults, permissions.length, anyIsEnough);
    } catch (error) {
      LxLog.e(TAG, `requestPermissionsFromUser failed: ${String(error)}`);
      return PermissionOutcome.DENIED;
    }
  }
}
