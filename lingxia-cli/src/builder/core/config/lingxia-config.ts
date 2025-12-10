import fs from 'fs';
import path from 'path';
import vm from 'vm';
import { createRequire } from 'module';
import type { ViewBuildConfig } from './view-build-schema.js';

export type FrameworkName = 'react' | 'vue';

export type PluginDescriptor =
  | string
  | {
      module: string;
      namedExport?: string;
      options?: any;
    };

export type PluginEntry = PluginDescriptor | any;

export interface NormalizedPluginDescriptor {
  module: string;
  namedExport?: string;
  options?: any;
}

export type NormalizedPluginSpec =
  | (NormalizedPluginDescriptor & { type?: 'descriptor' })
  | { plugin: any; module?: undefined };

export type PluginConfig =
  | PluginEntry[]
  | Partial<Record<FrameworkName, PluginEntry[]>>;

export interface LingxiaConfig {
  /**
   * Static asset directories copied during build.
   */
  staticDirs?: string[];
  /**
   * Module path aliases shared across logic/view builds.
   */
  alias?: Record<string, string>;
  /**
   * Additional source directories copied into the view build workspace.
   */
  sourceDirs?: string[];
  /**
   * Framework-specific view build overrides.
   */
  view?: LingxiaViewConfigOverrides;
  /**
   * Additional Vite plugins per framework.
   */
  plugins?: PluginConfig;
  /**
   * Placeholder for future config surface so we don't need breaking changes.
   */
  [key: string]: unknown;
}

export type LingxiaViewConfigOverrides = Partial<
  Record<FrameworkName, Partial<ViewBuildConfig>>
>;

export interface LingxiaConfigLoaderOptions {
  filename?: string;
}

const DEFAULT_CONFIG_FILENAME = 'lingxia.config.ts';
const cliRequire = createRequire(import.meta.url);

export function defineLingxiaConfig(config: LingxiaConfig): LingxiaConfig {
  return config;
}

export function loadLingxiaConfig(
  projectPath: string,
  options?: LingxiaConfigLoaderOptions
): LingxiaConfig | undefined {
  const configPath = path.join(
    projectPath,
    options?.filename ?? DEFAULT_CONFIG_FILENAME
  );

  if (!fs.existsSync(configPath)) {
    return undefined;
  }

  return readLingxiaTsConfig(configPath);
}

function readLingxiaTsConfig(filePath: string): LingxiaConfig | undefined {
  try {
    const source = fs.readFileSync(filePath, 'utf-8');
    const ts = loadTypescriptCompiler();
    const result = ts.transpileModule(source, {
      compilerOptions: {
        module: ts.ModuleKind.CommonJS,
        target: ts.ScriptTarget.ES2019,
        esModuleInterop: true
      },
      fileName: filePath,
      reportDiagnostics: true
    });

    if (result.diagnostics?.length) {
      const message = ts.formatDiagnosticsWithColorAndContext(
        result.diagnostics,
        {
          getCanonicalFileName: f => f,
          getCurrentDirectory: () => path.dirname(filePath),
          getNewLine: () => '\n'
        }
      );
      console.warn(`⚠️ Diagnostics while parsing ${filePath}:\n${message}`);
    }

    const exports = executeCommonJsModule(result.outputText, filePath);
    const config = exports?.default ?? exports;
    if (config && typeof config === 'object') {
      return config as LingxiaConfig;
    }
    return undefined;
  } catch (error) {
    console.warn(
      `⚠️ Failed to read Lingxia config from ${filePath}:`,
      error instanceof Error ? error.message : String(error)
    );
    return undefined;
  }
}

function executeCommonJsModule(code: string, filename: string): any {
  const module = { exports: {} as any };
  const localRequire = createRequire(filename);
  const context: vm.Context & Record<string, unknown> = {
    module,
    exports: module.exports,
    require: localRequire,
    __dirname: path.dirname(filename),
    __filename: filename,
    process,
    console,
    Buffer,
    setTimeout,
    clearTimeout,
    setInterval,
    clearInterval,
    setImmediate,
    clearImmediate
  };

  context.global = context;
  context.globalThis = context;

  vm.runInNewContext(code, context, { filename });
  return module.exports;
}

function loadTypescriptCompiler(): typeof import('typescript') {
  try {
    return cliRequire('typescript');
  } catch {
    throw new Error(
      "Cannot load 'typescript'. Please add it as a dependency to use lingxia.config.ts files."
    );
  }
}

export function extractViewOverrides(
  config: LingxiaConfig | undefined,
  framework?: FrameworkName
): Partial<ViewBuildConfig> | undefined {
  const overrides = config?.view;
  if (!overrides || typeof overrides !== 'object') {
    return undefined;
  }

  const frameworks: FrameworkName[] = ['react', 'vue'];

  // If overrides already look like a framework-specific record, pick the right one.
  if (framework) {
    const targeted = (overrides as Record<string, unknown>)[framework];
    if (targeted && typeof targeted === 'object') {
      return targeted as Partial<ViewBuildConfig>;
    }
  }

  // If overrides is a plain config (no react/vue keys), treat it as shared.
  const hasFrameworkKeys = frameworks.some(
    fw => Object.prototype.hasOwnProperty.call(overrides, fw)
  );
  if (!hasFrameworkKeys) {
    return overrides as Partial<ViewBuildConfig>;
  }

  // Fallback: return the first matching framework block if available.
  for (const fw of frameworks) {
    const block = (overrides as Record<string, unknown>)[fw];
    if (block && typeof block === 'object') {
      return block as Partial<ViewBuildConfig>;
    }
  }

  return undefined;
}

export function extractPluginSpecs(
  config?: LingxiaConfig
): Partial<Record<FrameworkName, NormalizedPluginSpec[]>> | undefined {
  const pluginsConfig = config?.plugins;
  if (!pluginsConfig) {
    return undefined;
  }

  const frameworks: FrameworkName[] = ['react', 'vue'];
  const normalized: Partial<Record<FrameworkName, NormalizedPluginSpec[]>> = {};
  let hasPlugins = false;

  const collectEntries = (entries: PluginEntry[] | undefined): NormalizedPluginSpec[] => {
    if (!entries || entries.length === 0) return [];
    const normalizedEntries: NormalizedPluginSpec[] = [];
    for (const entry of entries) {
      if (typeof entry === 'string') {
        const trimmed = entry.trim();
        if (trimmed.length > 0) {
          normalizedEntries.push({ module: trimmed });
        }
        continue;
      }
      if (
        entry &&
        typeof entry === 'object' &&
        typeof (entry as any).module === 'string'
      ) {
        normalizedEntries.push({
          module: (entry as any).module,
          namedExport:
            typeof (entry as any).namedExport === 'string'
              ? (entry as any).namedExport
              : undefined,
          options: (entry as any).options
        });
        continue;
      }
      if (entry !== undefined && entry !== null) {
        normalizedEntries.push({ plugin: entry });
      }
    }
    return normalizedEntries;
  };

  if (Array.isArray(pluginsConfig)) {
    const sharedEntries = collectEntries(pluginsConfig);
    if (sharedEntries.length > 0) {
      for (const framework of frameworks) {
        normalized[framework] = [...(normalized[framework] ?? []), ...sharedEntries];
      }
      hasPlugins = true;
    }
  } else if (typeof pluginsConfig === 'object') {
    for (const framework of frameworks) {
      const entries = collectEntries(
        Array.isArray(pluginsConfig[framework]) ? pluginsConfig[framework] : undefined
      );
      if (entries.length > 0) {
        normalized[framework] = [...(normalized[framework] ?? []), ...entries];
        hasPlugins = true;
      }
    }
  }

  return hasPlugins ? normalized : undefined;
}
