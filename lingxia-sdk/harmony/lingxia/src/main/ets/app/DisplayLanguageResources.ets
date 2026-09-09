import { resourceManager } from '@kit.LocalizationKit';
import { getDisplayLanguage } from 'liblingxia.so';

/**
 * SDK string in the product display language, not the process locale.
 * `$r('app.string.*')` follows the system locale, which can disagree with it.
 */
export function localizedString(resMgr: resourceManager.ResourceManager, name: string): string {
  try {
    const overrideConfig = resMgr.getOverrideConfiguration();
    overrideConfig.locale = localeForDisplayLanguage(getDisplayLanguage());
    return resMgr.getOverrideResourceManager(overrideConfig).getStringByNameSync(name);
  } catch (_) {
    return name;
  }
}

function localeForDisplayLanguage(tag: string): string {
  const normalized: string = tag.replace('_', '-').toLowerCase();
  return normalized === 'zh' || normalized.startsWith('zh-') ? 'zh_CN' : 'en_US';
}
