import path from 'path';
import type { BuildConfig } from '../config/lxapp-config.js';
import { loadLxappConfig } from '../config/lxapp-config.js';

const FALLBACK_SOURCE_DIRS = ['shared'] as const;

export const DEFAULT_SOURCE_DIRS = [...FALLBACK_SOURCE_DIRS];

export function resolveSourceDirs(
  projectPath: string,
  config?: BuildConfig
): string[] {
  const resolvedConfig = config ?? loadLxappConfig(projectPath);
  const dirs = Array.isArray(resolvedConfig?.sourceDirs)
    ? resolvedConfig.sourceDirs
    : null;

  if (!dirs) {
    return DEFAULT_SOURCE_DIRS;
  }

  const normalized = dirs
    .filter(dir => typeof dir === 'string')
    .map(dir => dir.trim())
    .filter(dir => dir.length > 0)
    .map(dir => dir.replace(/^[./\\]+/, '')) // normalize leading separators
    .map(dir => dir.split(path.sep).join('/'));

  if (normalized.length === 0) {
    return DEFAULT_SOURCE_DIRS;
  }

  return Array.from(new Set(normalized));
}
