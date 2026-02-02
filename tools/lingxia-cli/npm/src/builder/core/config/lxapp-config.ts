import fs from "fs";
import path from "path";
import vm from "vm";
import { createRequire } from "module";
import type { ViewBuildConfig } from "./view-build-schema.js";

export type FrameworkName = "react" | "vue" | "html";

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
  | (NormalizedPluginDescriptor & { type?: "descriptor" })
  | { plugin: any; module?: undefined };

export type PluginConfig =
  | PluginEntry[]
  | Partial<Record<FrameworkName, PluginEntry[]>>;

/**
 * Build configuration for lxapp and lxplugin projects.
 */
export interface BuildConfig {
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
   * Asset directory name for the view build.
   */
  assetDir?: string;
  /**
   * Framework-specific view build overrides.
   */
  view?: ViewConfigOverrides;
  /**
   * Additional Vite plugins per framework.
   */
  plugins?: PluginConfig;
}

export type ViewConfigOverrides = Partial<
  Record<FrameworkName, Partial<ViewBuildConfig>>
>;

const cliRequire = createRequire(import.meta.url);

/**
 * Helper function for type-safe config definition.
 */
export function defineConfig(config: BuildConfig): BuildConfig {
  return config;
}

/**
 * Load build config from project directory.
 * Only supports .ts format for consistency.
 *
 * @param projectPath - Project root directory
 * @param configName - Config file name without extension (e.g., 'lxapp.config' or 'lxplugin.config')
 */
export function loadBuildConfig(
  projectPath: string,
  configName: string,
): BuildConfig | undefined {
  const configPath = path.join(projectPath, `${configName}.ts`);

  if (!fs.existsSync(configPath)) {
    return undefined;
  }

  return readTsConfig(configPath);
}

/**
 * Load lxapp build config (lxapp.config.ts)
 */
export function loadLxappConfig(projectPath: string): BuildConfig | undefined {
  return loadBuildConfig(projectPath, "lxapp.config");
}

/**
 * Load lxplugin build config (lxplugin.config.ts)
 */
export function loadLxpluginConfig(
  projectPath: string,
): BuildConfig | undefined {
  return loadBuildConfig(projectPath, "lxplugin.config");
}

function readTsConfig(filePath: string): BuildConfig | undefined {
  try {
    const source = fs.readFileSync(filePath, "utf-8");
    const ts = loadTypescriptCompiler();

    const result = ts.transpileModule(source, {
      compilerOptions: {
        module: ts.ModuleKind.CommonJS,
        target: ts.ScriptTarget.ES2019,
        esModuleInterop: true,
      },
      fileName: filePath,
      reportDiagnostics: true,
    });

    if (result.diagnostics?.length) {
      const message = ts.formatDiagnosticsWithColorAndContext(
        result.diagnostics,
        {
          getCanonicalFileName: (f) => f,
          getCurrentDirectory: () => path.dirname(filePath),
          getNewLine: () => "\n",
        },
      );
      console.warn(`⚠️ Diagnostics while parsing ${filePath}:\n${message}`);
    }

    const exports = executeCommonJsModule(result.outputText, filePath);
    const config = exports?.default ?? exports;

    if (config && typeof config === "object") {
      return config as BuildConfig;
    }
    return undefined;
  } catch (error) {
    console.warn(
      `⚠️ Failed to read config from ${filePath}:`,
      error instanceof Error ? error.message : String(error),
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
    clearImmediate,
  };

  context.global = context;
  context.globalThis = context;

  vm.runInNewContext(code, context, { filename });
  return module.exports;
}

function loadTypescriptCompiler(): typeof import("typescript") {
  try {
    return cliRequire("typescript");
  } catch {
    throw new Error("Cannot load 'typescript'. Please add it as a dependency.");
  }
}

export function extractViewOverrides(
  config: BuildConfig | undefined,
  framework?: FrameworkName,
): Partial<ViewBuildConfig> | undefined {
  const overrides = config?.view;
  if (!overrides || typeof overrides !== "object") {
    return undefined;
  }

  const frameworks: FrameworkName[] = ["react", "vue"];

  const hasFrameworkKeys = frameworks.some((fw) =>
    Object.prototype.hasOwnProperty.call(overrides, fw),
  );

  if (hasFrameworkKeys && framework) {
    const targeted = (overrides as Record<string, unknown>)[framework];
    if (targeted && typeof targeted === "object") {
      return targeted as Partial<ViewBuildConfig>;
    }
  }

  if (!hasFrameworkKeys) {
    return overrides as Partial<ViewBuildConfig>;
  }

  if (framework) {
    for (const fw of frameworks) {
      const block = (overrides as Record<string, unknown>)[fw];
      if (block && typeof block === "object") {
        return block as Partial<ViewBuildConfig>;
      }
    }
  }

  return undefined;
}

export function extractPluginSpecs(
  config?: BuildConfig,
): Partial<Record<FrameworkName, NormalizedPluginSpec[]>> | undefined {
  const pluginsConfig = config?.plugins;
  if (!pluginsConfig) {
    return undefined;
  }

  const frameworks: FrameworkName[] = ["react", "vue"];
  const normalized: Partial<Record<FrameworkName, NormalizedPluginSpec[]>> = {};
  let hasPlugins = false;

  const collectEntries = (
    entries: PluginEntry[] | undefined,
  ): NormalizedPluginSpec[] => {
    if (!entries || entries.length === 0) return [];
    const normalizedEntries: NormalizedPluginSpec[] = [];
    for (const entry of entries) {
      if (typeof entry === "string") {
        const trimmed = entry.trim();
        if (trimmed.length > 0) {
          normalizedEntries.push({ module: trimmed });
        }
        continue;
      }
      if (
        entry &&
        typeof entry === "object" &&
        typeof (entry as any).module === "string"
      ) {
        const moduleName = (entry as any).module;
        if (moduleName.trim().length > 0) {
          normalizedEntries.push({
            module: moduleName,
            namedExport:
              typeof (entry as any).namedExport === "string"
                ? (entry as any).namedExport
                : undefined,
            options: (entry as any).options,
          });
        }
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
        normalized[framework] = [
          ...(normalized[framework] ?? []),
          ...sharedEntries,
        ];
      }
      hasPlugins = true;
    }
  } else if (typeof pluginsConfig === "object") {
    for (const framework of frameworks) {
      const entries = collectEntries(
        Array.isArray(pluginsConfig[framework])
          ? pluginsConfig[framework]
          : undefined,
      );
      if (entries.length > 0) {
        normalized[framework] = [...(normalized[framework] ?? []), ...entries];
        hasPlugins = true;
      }
    }
  }

  return hasPlugins ? normalized : undefined;
}

// Type aliases for backward compatibility
export type LxappConfig = BuildConfig;
