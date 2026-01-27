import fs from 'fs';
import path from 'path';
export interface BuildConfig {
  staticDirs?: string[];
  alias?: Record<string, string>;
  sourceDirs?: string[];
  assetDir?: string;
}

export function loadLxappBuildConfig(projectPath: string): BuildConfig | undefined {
  const configPath = path.join(projectPath, 'lxapp.config.json');
  if (!fs.existsSync(configPath)) {
    return undefined;
  }

  try {
    const raw = fs.readFileSync(configPath, 'utf-8');
    const parsed = JSON.parse(raw);
    if (!parsed || typeof parsed !== 'object' || Array.isArray(parsed)) {
      throw new Error('lxapp.config.json must be a JSON object');
    }
    const rawConfig = parsed as Record<string, unknown>;
    const alias =
      rawConfig.alias && typeof rawConfig.alias === 'object' && !Array.isArray(rawConfig.alias)
        ? (rawConfig.alias as Record<string, string>)
        : undefined;
    return {
      staticDirs: Array.isArray(rawConfig.staticDirs)
        ? (rawConfig.staticDirs as string[])
        : undefined,
      alias,
      sourceDirs: Array.isArray(rawConfig.sourceDirs)
        ? (rawConfig.sourceDirs as string[])
        : undefined,
      assetDir: typeof rawConfig.assetDir === 'string' ? rawConfig.assetDir : undefined
    };
  } catch (error) {
    throw new Error(
      `Invalid JSON: lxapp.config.json\n${error instanceof Error ? error.message : String(error)}`
    );
  }
}
