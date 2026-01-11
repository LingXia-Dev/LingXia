import type { ViewBuildConfig } from './view-build-schema.js';

export const DEFAULT_ASSET_DIR = 'assets';

function createDefaultConfig(): Record<'react' | 'vue', ViewBuildConfig> {
  const baseMultiOutput = (assetDir: string) => ({
    entryFileNames: 'pages/[name]/[name].js',
    chunkFileNames: `${assetDir}/vendor-[hash].js`,
    assetFileNames: `${assetDir}/[name].[ext]`,
    manualChunks: null
  });

  return {
    react: {
      output: {
        multi: baseMultiOutput(DEFAULT_ASSET_DIR)
      },
      assetDir: DEFAULT_ASSET_DIR,
      cssCodeSplitMulti: true,
      target: 'es2020',
      esbuild: { jsx: 'automatic' },
      minifyStrategy: 'esbuild'
    },
    vue: {
      output: {
        multi: baseMultiOutput(DEFAULT_ASSET_DIR)
      },
      assetDir: DEFAULT_ASSET_DIR,
      cssCodeSplitMulti: true,
      target: 'es2020',
      minifyStrategy: 'esbuild'
    }
  };
}

const defaultConfig: Record<'react' | 'vue', ViewBuildConfig> =
  createDefaultConfig();

import {
  extractViewOverrides,
  loadLingxiaConfig,
  type FrameworkName
} from './lingxia-config.js';

export class ViewConfigManager {
  private projectPath: string;
  private overrides?: Partial<ViewBuildConfig>;

  constructor(projectPath: string, overrides?: Partial<ViewBuildConfig>) {
    this.projectPath = projectPath;
    this.overrides = overrides;
  }

  getFrameworkConfig(framework: 'react' | 'vue'): ViewBuildConfig {
    const base = defaultConfig[framework];
    const extra = this.overrides;
    if (!extra) return base;
    return mergeViewConfig(base, extra);
  }
}

export function resolveUserViewConfig(
  projectPath: string,
  framework: FrameworkName
): Partial<ViewBuildConfig> | undefined {
  const config = loadLingxiaConfig(projectPath);
  return extractViewOverrides(config, framework);
}

function mergeViewConfig(
  base: ViewBuildConfig,
  extra: Partial<ViewBuildConfig>
): ViewBuildConfig {
  const assetDir = extra.assetDir ?? base.assetDir ?? DEFAULT_ASSET_DIR;
  const multiOutput = {
    ...base.output.multi,
    ...(extra.output?.multi ?? {})
  };

  if (!extra.output?.multi?.assetFileNames) {
    multiOutput.assetFileNames = `${assetDir}/[name].[ext]`;
  }

  if (!extra.output?.multi?.chunkFileNames) {
    multiOutput.chunkFileNames = `${assetDir}/[name]-[hash].js`;
  }

  return {
    ...base,
    ...extra,
    output: {
      multi: multiOutput
    },
    assetDir,
    cssCodeSplitMulti: extra.cssCodeSplitMulti ?? base.cssCodeSplitMulti,
    target: extra.target ?? base.target,
    esbuild: { ...base.esbuild, ...(extra.esbuild ?? {}) },
    minifyStrategy: extra.minifyStrategy ?? base.minifyStrategy,
    resolvePlugins: extra.resolvePlugins ?? base.resolvePlugins,
    cssConfig: extra.cssConfig ?? base.cssConfig
  };
}
