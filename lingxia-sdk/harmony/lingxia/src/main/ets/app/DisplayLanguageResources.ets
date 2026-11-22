import { resourceManager } from '@kit.LocalizationKit';
import { getDisplayLanguage } from 'liblingxia.so';
import { LxLog } from './LxLog';

const TAG = 'DisplayLanguageResources';

/**
 * SDK string in the product display language, not the process locale.
 * `$r('app.string.*')` follows the system locale, which can disagree with it.
 */
export function localizedString(resMgr: resourceManager.ResourceManager, name: string): string {
  try {
    const overrideConfig = resMgr.getOverrideConfiguration();
    overrideConfig.locale = localeForDisplayLanguage(getDisplayLanguage());
    return resMgr.getOverrideResourceManager(overrideConfig).getStringByNameSync(name);
  } catch (error) {
    LxLog.w(TAG, `localizedString(${name}) failed: ${String(error)}`);
    return name;
  }
}

// The host has narrowed it to a product language; resource lookup picks the nearest qualifier.
function localeForDisplayLanguage(tag: string): string {
  return tag.replace(/-/g, '_');
}
