import common from '@ohos.app.ability.common';
import { preferences } from '@kit.ArkData';
import { getCurrentLxapp, type CurrentLxApp } from 'liblingxia.so';
import { LxLog } from './LxLog';

/** The lx feature that needs a runtime permission. */
export enum PermissionFeature {
  CAMERA = 'CAMERA',
  SCAN_CODE = 'SCAN_CODE',
  SAVE_MEDIA = 'SAVE_MEDIA',
  LOCATION = 'LOCATION',
  WIFI = 'WIFI',
  NOTIFICATION = 'NOTIFICATION',
}

export enum PermissionOutcome {
  GRANTED = 'GRANTED',
  DENIED = 'DENIED',
  /** Denied and the system will not prompt again; only Settings can grant it. */
  DENIED_PERMANENTLY = 'DENIED_PERMANENTLY',
}

/** A system permission prompt LingXia is about to show. */
export class PermissionPrompt {
  readonly context: common.UIAbilityContext;
  readonly feature: PermissionFeature;
  /** Missing `ohos.permission.*` names the prompt asks for; empty for notifications. */
  readonly permissions: string[];
  /** The lxapp that triggered it, when one is foreground. */
  readonly appId: string | undefined;
  /**
   * The user denied it before, so the regular system prompt will not show.
   * [proceed] then opens the system re-grant sheet instead.
   */
  readonly blocked: boolean;
  private readonly decide: (allowed: boolean) => void;

  constructor(context: common.UIAbilityContext, feature: PermissionFeature, permissions: string[],
    appId: string | undefined, blocked: boolean, decide: (allowed: boolean) => void) {
    this.context = context;
    this.feature = feature;
    this.permissions = permissions;
    this.appId = appId;
    this.blocked = blocked;
    this.decide = decide;
  }

  /** Show the system prompt. */
  proceed(): void {
    this.decide(true);
  }

  /** Skip the system prompt; the lx API fails as permission denied. */
  deny(): void {
    this.decide(false);
  }
}

/**
 * Host hook, called each time an lx API needs a permission it lacks (never
 * once granted), including after a denial (`prompt.blocked`). Prompts are
 * gated one at a time.
 */
export interface PermissionGate {
  /** Call `prompt.proceed()` or `prompt.deny()`, now or later; only the first call counts. */
  beforePrompt(prompt: PermissionPrompt): void;

  /** The system prompt closed. Not called after `prompt.deny()`. */
  afterPrompt(prompt: PermissionPrompt, outcome: PermissionOutcome): void;
}

const ASKED_STORE = 'lingxia_permission_prompts';

/** SDK side of [PermissionGate]. */
export class PermissionGates {
  static gate: PermissionGate | null = null;
  private static tail: Promise<void> = Promise.resolve();

  static isGated(): boolean {
    return PermissionGates.gate !== null;
  }

  /**
   * Resolves whether [show] ran and reported GRANTED. Without a gate, [show]
   * runs directly.
   */
  static run(context: common.UIAbilityContext, feature: PermissionFeature, permissions: string[],
    blocked: boolean, show: () => Promise<PermissionOutcome>): Promise<boolean> {
    const gate = PermissionGates.gate;
    if (gate === null) {
      return show().then((outcome: PermissionOutcome) => outcome === PermissionOutcome.GRANTED);
    }
    const result = PermissionGates.tail.then(() => PermissionGates.gateOne(gate, context, feature, permissions,
      blocked, show));
    PermissionGates.tail = result.then(() => {}, () => {});
    return result;
  }

  private static gateOne(gate: PermissionGate, context: common.UIAbilityContext, feature: PermissionFeature,
    permissions: string[], blocked: boolean, show: () => Promise<PermissionOutcome>): Promise<boolean> {
    return new Promise<boolean>((resolve: (granted: boolean) => void) => {
      let decided = false;
      let prompt: PermissionPrompt | null = null;
      prompt = new PermissionPrompt(context, feature, permissions, PermissionGates.currentAppId(), blocked,
        (allowed: boolean) => {
          if (decided) {
            return;
          }
          decided = true;
          if (!allowed) {
            resolve(false);
            return;
          }
          show().then((outcome: PermissionOutcome) => {
            gate.afterPrompt(prompt!, outcome);
            resolve(outcome === PermissionOutcome.GRANTED);
          }, () => {
            gate.afterPrompt(prompt!, PermissionOutcome.DENIED);
            resolve(false);
          });
        });
      gate.beforePrompt(prompt);
    });
  }

  /** Permissions LingXia has prompted for before; Harmony never re-prompts a denial. */
  static asked(context: common.UIAbilityContext, permission: string): boolean {
    try {
      const store = preferences.getPreferencesSync(context, { name: ASKED_STORE });
      return store.getSync(permission, false) as boolean;
    } catch (_error) {
      return false;
    }
  }

  static markAsked(context: common.UIAbilityContext, permissions: string[]): void {
    try {
      const store = preferences.getPreferencesSync(context, { name: ASKED_STORE });
      permissions.forEach((permission: string) => store.putSync(permission, true));
      store.flush();
    } catch (error) {
      LxLog.w('LingXia.PermissionGate', `markAsked failed: ${String(error)}`);
    }
  }

  private static currentAppId(): string | undefined {
    try {
      const current: CurrentLxApp | null = getCurrentLxapp();
      const appId: string = current?.appid ?? '';
      return appId.length > 0 ? appId : undefined;
    } catch (_error) {
      return undefined;
    }
  }
}
