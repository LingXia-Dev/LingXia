import {
  AbilityConstant,
  common,
  Configuration,
  ConfigurationConstant,
  EnvironmentCallback
} from '@kit.AbilityKit';
import { hilog } from '@kit.PerformanceAnalysisKit';
import { initializeHostAppearance, onHostAppearanceChanged } from 'liblingxia.so';

const DOMAIN = 0x0000;
const TAG = 'LingXia.Appearance';

export class HostAppearance {
  private static context: common.UIAbilityContext | null = null;
  private static preference: number = 0;
  private static callbackId: number | null = null;
  private static lastEffectiveDark: boolean | null = null;
  private static readonly environmentCallback: EnvironmentCallback = {
    onConfigurationUpdated: (config: Configuration): void => {
      if (HostAppearance.preference !== 0) {
        return;
      }
      HostAppearance.emit(config.colorMode === ConfigurationConstant.ColorMode.COLOR_MODE_DARK);
    },
    onMemoryLevel: (_level: AbilityConstant.MemoryLevel): void => {}
  };

  public static initialize(context: common.UIAbilityContext): void {
    HostAppearance.context = context;
    const dark = HostAppearance.contextIsDark();
    HostAppearance.lastEffectiveDark = dark;
    initializeHostAppearance(dark);
    if (HostAppearance.callbackId === null) {
      HostAppearance.callbackId = context.getApplicationContext()
        .on('environment', HostAppearance.environmentCallback);
    }
  }

  public static setPreference(raw: number): boolean {
    if (!Number.isInteger(raw) || raw < 0 || raw > 2 || !HostAppearance.context) {
      return false;
    }
    const previous = HostAppearance.preference;
    HostAppearance.preference = raw;
    const colorMode = HostAppearance.colorMode(raw);
    try {
      HostAppearance.context.getApplicationContext().setColorMode(colorMode);
      HostAppearance.emit(HostAppearance.effectiveDark(), true);
      return true;
    } catch (error) {
      HostAppearance.preference = previous;
      hilog.error(DOMAIN, TAG, `Failed to set host appearance: ${error}`);
      return false;
    }
  }

  private static colorMode(raw: number): ConfigurationConstant.ColorMode {
    if (raw === 1) {
      return ConfigurationConstant.ColorMode.COLOR_MODE_LIGHT;
    }
    if (raw === 2) {
      return ConfigurationConstant.ColorMode.COLOR_MODE_DARK;
    }
    return ConfigurationConstant.ColorMode.COLOR_MODE_NOT_SET;
  }

  private static contextIsDark(): boolean {
    return HostAppearance.context?.config?.colorMode ===
      ConfigurationConstant.ColorMode.COLOR_MODE_DARK;
  }

  private static effectiveDark(): boolean {
    if (HostAppearance.preference === 1) {
      return false;
    }
    if (HostAppearance.preference === 2) {
      return true;
    }
    return HostAppearance.contextIsDark();
  }

  private static emit(dark: boolean, force: boolean = false): void {
    if (!force && dark === HostAppearance.lastEffectiveDark) {
      return;
    }
    HostAppearance.lastEffectiveDark = dark;
    onHostAppearanceChanged(HostAppearance.preference, dark);
  }
}
