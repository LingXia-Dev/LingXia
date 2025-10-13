export interface ViewBuildConfig {
  output: {
    single: Record<string, unknown>;
    multi: Record<string, unknown>;
  };
  cssCodeSplitSingle: boolean;
  cssCodeSplitMulti: boolean;
  target?: string;
  esbuild?: Record<string, unknown>;
  minifyStrategy?: 'esbuild' | 'terser' | boolean;
  resolvePlugins?: (framework: 'react' | 'vue') => Promise<any[]>;
  cssConfig?:
    | false
    | ((
        buildDir: string
      ) => Promise<{ postcss?: { plugins?: any[] } } | undefined | null>);
}

const defaultConfig: Record<'react' | 'vue', ViewBuildConfig> = {
  react: {
    output: {
      single: {
        entryFileNames: 'main.js',
        chunkFileNames: 'chunks/[name]-[hash].js',
        assetFileNames: 'assets/[name].[ext]'
      },
      multi: {
        entryFileNames: 'pages/[name]/[name].js',
        chunkFileNames: 'assets/[name]-[hash].js',
        assetFileNames: 'assets/[name].[ext]',
        manualChunks: null
      }
    },
    cssCodeSplitSingle: false,
    cssCodeSplitMulti: false,
    target: 'es2015',
    esbuild: { jsx: 'automatic' },
    minifyStrategy: 'esbuild'
  },
  vue: {
    output: {
      single: {
        entryFileNames: 'main.js',
        chunkFileNames: 'chunks/[name]-[hash].js',
        assetFileNames: 'assets/[name].[ext]'
      },
      multi: {
        entryFileNames: 'pages/[name]/[name].js',
        chunkFileNames: 'assets/[name]-[hash].js',
        assetFileNames: 'assets/[name].[ext]',
        manualChunks: null
      }
    },
    cssCodeSplitSingle: false,
    cssCodeSplitMulti: false,
    target: 'es2015',
    minifyStrategy: 'esbuild'
  }
};

export class ViewConfigManager {
  private projectPath: string;
  private overrides?: Record<'react' | 'vue', Partial<ViewBuildConfig>>;

  constructor(
    projectPath: string,
    overrides?: Record<'react' | 'vue', Partial<ViewBuildConfig>>
  ) {
    this.projectPath = projectPath;
    this.overrides = overrides;
  }

  getFrameworkConfig(framework: 'react' | 'vue'): ViewBuildConfig {
    const base = defaultConfig[framework];
    const extra = this.overrides?.[framework];
    if (!extra) return base;
    return mergeViewConfig(base, extra);
  }
}

export function resolveUserViewConfig(
  projectPath: string
): Record<'react' | 'vue', Partial<ViewBuildConfig>> | undefined {
  // TODO: load lingxia.config.ts in future iterations.
  void projectPath;
  return undefined;
}

function mergeViewConfig(
  base: ViewBuildConfig,
  extra: Partial<ViewBuildConfig>
): ViewBuildConfig {
  return {
    ...base,
    ...extra,
    output: {
      single: { ...base.output.single, ...(extra.output?.single ?? {}) },
      multi: { ...base.output.multi, ...(extra.output?.multi ?? {}) }
    },
    cssCodeSplitSingle:
      extra.cssCodeSplitSingle ?? base.cssCodeSplitSingle,
    cssCodeSplitMulti: extra.cssCodeSplitMulti ?? base.cssCodeSplitMulti,
    target: extra.target ?? base.target,
    esbuild: { ...base.esbuild, ...(extra.esbuild ?? {}) },
    minifyStrategy: extra.minifyStrategy ?? base.minifyStrategy,
    resolvePlugins: extra.resolvePlugins ?? base.resolvePlugins,
    cssConfig: extra.cssConfig ?? base.cssConfig
  };
}
