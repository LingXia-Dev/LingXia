import fg from 'fast-glob';
import path from 'path';
import type { BuildConfig } from '../config/lxapp-config.js';
import { loadLxappConfig } from '../config/lxapp-config.js';

const FALLBACK_STATIC_DIRS = ['public'] as const;

export const DEFAULT_STATIC_DIRS = [...FALLBACK_STATIC_DIRS];

export function resolveStaticDirs(
  projectPath: string,
  config?: BuildConfig
): string[] {
  const resolvedConfig = config ?? loadLxappConfig(projectPath);
  const dirs = Array.isArray(resolvedConfig?.staticDirs)
    ? resolvedConfig.staticDirs
    : null;
  if (!dirs) return DEFAULT_STATIC_DIRS;

  const normalized = dirs
    .filter(dir => typeof dir === 'string')
    .map(dir => dir.trim())
    .filter(dir => dir.length > 0);

  if (normalized.length === 0) {
    return DEFAULT_STATIC_DIRS;
  }

  const expanded = normalized.flatMap(entry => expandStaticDirEntry(projectPath, entry));
  if (expanded.length === 0) {
    return DEFAULT_STATIC_DIRS;
  }

  return Array.from(new Set(expanded));
}

function expandStaticDirEntry(projectPath: string, entry: string): string[] {
  if (!looksLikeGlob(entry)) {
    return [entry];
  }

  try {
    const matches = fg.sync(entry, {
      cwd: projectPath,
      onlyDirectories: true,
      dot: false
    });

    if (matches.length === 0) {
      // Silently skip unmatched globs - this is expected when optional static dirs don't exist
      return [];
    }

    // Normalize to posix-style relative paths regardless of platform separators
    return matches.map(match => match.split(path.sep).join('/'));
  } catch (error) {
    console.warn(
      `⚠️ Failed to expand staticDirs glob ${entry}:`,
      error instanceof Error ? error.message : String(error)
    );
    return [];
  }
}

function looksLikeGlob(value: string): boolean {
  return /[*?\[\]{}()!+@]/.test(value);
}
