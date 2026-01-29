import path from 'path';
import type { LingxiaConfig } from './lingxia-config.js';
import type { BuildConfig } from './build-config.js';
import { loadLingxiaConfig } from './lingxia-config.js';

export type AliasMap = Record<string, string>;

// Use @/ instead of @lingxia to avoid conflict with npm @lingxia/* packages
const DEFAULT_ROOT_ALIAS = '@';

function normalizeAliases(
  projectPath: string,
  config?: LingxiaConfig | BuildConfig | undefined
): AliasMap {
  if (!config?.alias || typeof config.alias !== 'object') {
    return {};
  }

  const entries = Object.entries(config.alias).filter(entry => {
    const [key, value] = entry;
    return (
      typeof key === 'string' &&
      key.trim().length > 0 &&
      typeof value === 'string' &&
      value.trim().length > 0
    );
  });

  const alias: AliasMap = {};
  for (const [key, value] of entries) {
    alias[key] = path.resolve(projectPath, value);
  }

  return alias;
}

export function resolveAliasMap(
  projectPath: string,
  config?: BuildConfig
): AliasMap {
  const resolvedConfig = config ?? loadLingxiaConfig(projectPath);
  const alias = normalizeAliases(projectPath, resolvedConfig);
  if (!alias[DEFAULT_ROOT_ALIAS]) {
    alias[DEFAULT_ROOT_ALIAS] = projectPath;
  }
  return alias;
}
