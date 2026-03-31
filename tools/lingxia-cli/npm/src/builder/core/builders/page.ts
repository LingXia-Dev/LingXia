import * as fs from "fs";
import * as path from "path";
import { createRequire } from "module";
import { pathToFileURL } from "url";
import type { BuildOptions, Page, PageFiles } from "../../types/index.js";
import { FileUtils } from "../utils/file.js";
import { TemplateManager } from "../template.js";
import type { PageBridgeMethod } from "../template.js";
import { FrameworkFactory } from "../frameworks/factory.js";
import type { ViewBuildConfig } from "../config/view-build-schema.js";
import { ViewConfigManager, DEFAULT_ASSET_DIR } from "../config/view-config.js";
import {
  extractViewOverrides,
  extractPluginSpecs,
  loadLxappConfig,
  type NormalizedPluginSpec,
  type NormalizedPluginDescriptor,
} from "../config/lxapp-config.js";
import { readProjectFramework } from "../config/framework.js";
import type { ProjectFramework } from "../config/framework.js";
import { DEFAULT_STATIC_DIRS } from "../constants/static-dirs.js";
import {
  DEFAULT_SOURCE_DIRS,
  resolveSourceDirs,
} from "../constants/source-dirs.js";
import { resolveAliasMap } from "../config/alias-config.js";
import type { BuildConfig } from "../config/lxapp-config.js";

export class PageProcessor {
  private projectPath: string;
  private outputDir: string;
  private fileUtils: FileUtils;
  private templateManager: TemplateManager;
  private viewConfigManager: ViewConfigManager;
  private staticDirs: string[];
  private alias: Record<string, string>;
  private sourceDirs: string[];
  private pluginSpecs?: Partial<
    Record<"react" | "vue", NormalizedPluginSpec[]>
  >;
  private projectRequire: NodeJS.Require;
  private framework: ProjectFramework;

  constructor(
    projectPath: string,
    outputDir: string,
    staticDirs: string[] = DEFAULT_STATIC_DIRS,
    buildConfig?: BuildConfig,
    framework?: ProjectFramework,
  ) {
    this.projectPath = projectPath;
    this.outputDir = outputDir;
    this.fileUtils = new FileUtils();
    this.templateManager = new TemplateManager();
    // Use provided framework or auto-detect
    this.framework = framework ?? readProjectFramework(projectPath);
    // Use buildConfig if provided, otherwise load from lxapp.config.ts
    const lingxiaConfig = buildConfig ?? loadLxappConfig(projectPath);
    const viewOverrides = extractViewOverrides(
      lingxiaConfig as any,
      this.framework,
    );
    const combinedOverrides =
      lingxiaConfig?.assetDir && !viewOverrides?.assetDir
        ? { ...viewOverrides, assetDir: lingxiaConfig.assetDir }
        : viewOverrides;
    this.pluginSpecs = extractPluginSpecs(lingxiaConfig as any);
    this.viewConfigManager = new ViewConfigManager(
      projectPath,
      combinedOverrides,
    );
    this.staticDirs = staticDirs;
    this.alias = resolveAliasMap(projectPath, buildConfig);
    this.sourceDirs =
      resolveSourceDirs(projectPath, buildConfig) ?? DEFAULT_SOURCE_DIRS;
    this.projectRequire = this.createProjectRequire(projectPath);
  }

  /**
   * Batch build multiple pages for a single framework using Vite multi-entry.
   * Writes a dedicated multi-entry vite.config.js (no framework API changes),
   * installs once, builds once, then normalizes per-entry to existing processor
   * expectations by temporarily mapping <entry>.html/js to index.html/main.js.
   */
  async buildPagesBatch(
    framework: "react" | "vue",
    items: { page: Page; pageFiles: PageFiles; pageFunctions: PageBridgeMethod[] }[],
    options: BuildOptions = {},
  ): Promise<void> {
    if (framework !== this.framework) {
      throw new Error(
        `Project configured for ${this.framework} views, but attempted to build ${framework} pages.`,
      );
    }
    if (items.length === 0) return;
    const processor = FrameworkFactory.createProcessor(
      framework,
      this.projectPath,
      this.outputDir,
    );

    const buildDir = path.join(
      this.projectPath,
      ".lingxia",
      "build",
      `view-${framework}`,
    );
    this.fileUtils.cleanDirectory(buildDir);

    // Shared package.json: copy from root and override build script for Vite
    // Copy shared assets/config
    await this.copySourceDirectories(buildDir);
    await this.copyStaticDirectories(buildDir);

    // Prepare per-page subdirs and collect multi-entry inputs
    const inputs: Record<string, string> = {};
    const entryNameByPagePath: Record<string, string> = {};
    for (const { page, pageFiles, pageFunctions } of items) {
      const entryName =
        (page as any).name ||
        path.dirname(page.path).replace(/^pages\//, "") ||
        path.basename(page.path, path.extname(page.path));
      const subDir = path.join(buildDir, "pages", entryName);
      this.fileUtils.ensureDirectory(subDir);

      await processor.setupBuild(subDir, page, pageFiles, pageFunctions);
      inputs[entryName] = path.join(subDir, "index.html");
      entryNameByPagePath[page.path] = entryName;
    }

    const frameworkConfig =
      this.viewConfigManager.getFrameworkConfig(framework);
    const assetDir = frameworkConfig.assetDir ?? DEFAULT_ASSET_DIR;

    // CLI --target option overrides config
    const target = options.target ?? frameworkConfig.target;
    const esbuild = frameworkConfig.esbuild;

    console.log(`  ⏳ Bundling ${framework} view pages with Vite...`);
    await this.runViteBuild(buildDir, framework, {
      options,
      inputs,
      frameworkConfig,
      output: frameworkConfig.output.multi,
      cssCodeSplit: frameworkConfig.cssCodeSplitMulti,
      target,
      esbuild,
      alias: this.alias,
    });
    console.log(`  ✔ Vite bundle finished for ${framework} pages`);

    const distDir = path.join(buildDir, "dist");

    // Copy assets from build dist to final output
    const buildAssetsDir = path.join(distDir, assetDir);
    if (fs.existsSync(buildAssetsDir)) {
      const finalAssetsDir = path.join(this.outputDir, assetDir);
      this.fileUtils.ensureDirectory(finalAssetsDir);
      await this.fileUtils.copyDirectory(buildAssetsDir, finalAssetsDir);
    }
    console.log(`  ✔ Copied shared ${framework} view assets`);

    // For each entry, pass explicit file paths to the processor
    for (const { page, pageFiles, pageFunctions } of items) {
      const entryName = entryNameByPagePath[page.path];

      // Determine relative paths based on Vite output structure
      const entryHtml = path.join("pages", entryName, "index.html");
      const entryJs = path.join("pages", entryName, `${entryName}.js`);

      // Full bridge is bundled via __page_bridge__ module import.
      // Inline script only needs metadata (names) for early access.
      const bridgeScript =
        this.templateManager.generateBridgeMetadata(pageFunctions);
      await processor.generateOutput(
        page,
        pageFiles,
        {
          distDir,
          assetDir,
          entryHtml,
          entryJs,
        },
        bridgeScript,
      );
    }
    console.log(`  ✔ Finalized ${items.length} ${framework} page output(s)`);
  }

  private async runViteBuild(
    buildDir: string,
    framework: "react" | "vue",
    config: {
      options: BuildOptions;
      inputs: Record<string, string>;
      output: Record<string, unknown>;
      cssCodeSplit?: boolean;
      esbuild?: Record<string, unknown>;
      target?: string;
      frameworkConfig: ViewBuildConfig;
      alias?: Record<string, string>;
    },
  ): Promise<void> {
    const { build } = await import("vite");
    const plugins = await this.resolveFrameworkPlugins(
      framework,
      config.frameworkConfig,
    );
    const css = await this.createCssConfig(buildDir, config.frameworkConfig);
    const isProd = Boolean(config.options.release);
    const isDev = !isProd;

    await build({
      configFile: false,
      root: buildDir,
      base: "/", // Use absolute paths in HTML output
      logLevel: "warn",
      mode: isDev ? "development" : isProd ? "production" : undefined,
      plugins,
      css,
      resolve: {
        dedupe: ["react", "react-dom", "vue"],
        alias: Object.entries(config.alias ?? {}).map(
          ([find, replacement]) => {
            // Bare "@" alias maps to "@/" prefix so it doesn't collide
            // with scoped npm packages like @lingxia/*
            if (find === "@") {
              const normalized = replacement.endsWith("/")
                ? replacement
                : `${replacement}/`;
              return {
                find: /^@\//,
                replacement: normalized,
              };
            }
            return {
              find,
              replacement,
            };
          },
        ),
      },
      esbuild: config.esbuild && Object.keys(config.esbuild).length > 0 ? config.esbuild : undefined,
      build: {
        outDir: path.join(buildDir, "dist"),
        emptyOutDir: true,
        rollupOptions: {
          input: config.inputs,
          output: config.output,
        },
        cssCodeSplit: config.cssCodeSplit ?? true,
        target: config.target,
        minify: isProd
          ? (config.frameworkConfig.minifyStrategy ?? "esbuild")
          : false,
        sourcemap: isDev,
      },
    });
  }

  private async resolveFrameworkPlugins(
    framework: "react" | "vue",
    config: ViewBuildConfig,
  ) {
    const pluginFactories = await config.resolvePlugins?.(framework);
    let plugins: any[] | undefined;
    if (pluginFactories && pluginFactories.length > 0) {
      plugins = [...pluginFactories];
    } else if (framework === "react") {
      const reactModule = await import("@vitejs/plugin-react");
      const pluginFactory = (reactModule as any).default ?? reactModule;
      plugins = [pluginFactory()];
    } else {
      const vueModule = await import("@vitejs/plugin-vue");
      const pluginFactory = (vueModule as any).default ?? vueModule;
      plugins = [pluginFactory()];
    }

    const userPlugins = await this.loadUserPlugins(framework);
    if (userPlugins.length > 0) {
      plugins.push(...userPlugins);
    }
    return plugins;
  }

  private async createCssConfig(buildDir: string, config: ViewBuildConfig) {
    if (config.cssConfig === false) {
      return undefined;
    }
    if (typeof config.cssConfig === "function") {
      return config.cssConfig(buildDir);
    }
    return undefined;
  }

  private async copySourceDirectories(buildDir: string): Promise<void> {
    for (const dir of this.sourceDirs) {
      const sourceDir = path.join(this.projectPath, dir);
      if (fs.existsSync(sourceDir)) {
        const destDir = path.join(buildDir, dir);
        await this.fileUtils.copyDirectory(sourceDir, destDir);
      }
    }
  }

  private async copyStaticDirectories(buildDir: string): Promise<void> {
    for (const dirName of this.staticDirs) {
      const sourceDir = path.join(this.projectPath, dirName);
      if (fs.existsSync(sourceDir)) {
        const destDir = path.join(buildDir, dirName);
        await this.fileUtils.copyDirectory(sourceDir, destDir);
      }
    }
  }

  private async loadUserPlugins(framework: "react" | "vue"): Promise<any[]> {
    const specs = this.pluginSpecs?.[framework];
    if (!specs || specs.length === 0) {
      return [];
    }

    const plugins: any[] = [];
    for (const spec of specs) {
      if ("plugin" in spec && spec.plugin) {
        plugins.push(spec.plugin);
      } else {
        plugins.push(await this.instantiatePlugin(spec as any));
      }
    }
    return plugins;
  }

  private async instantiatePlugin(
    spec: NormalizedPluginDescriptor,
  ): Promise<any> {
    const resolvedPath = this.resolveModulePath(spec.module);
    const moduleUrl = pathToFileURL(resolvedPath).href;
    const imported = await import(moduleUrl);
    const factory = spec.namedExport
      ? imported[spec.namedExport]
      : (imported.default ?? imported);
    if (typeof factory !== "function") {
      throw new Error(
        `Plugin module "${spec.module}" must export a function (default or named) returning a Vite plugin.`,
      );
    }
    return await factory(spec.options);
  }

  private resolveModulePath(moduleId: string): string {
    try {
      return this.projectRequire.resolve(moduleId);
    } catch {
      throw new Error(
        `Cannot resolve plugin module "${moduleId}" from project ${this.projectPath}.`,
      );
    }
  }

  private createProjectRequire(projectPath: string): NodeJS.Require {
    const candidateFiles = ["package.json", "lxapp.config.ts"];
    for (const file of candidateFiles) {
      const fullPath = path.join(projectPath, file);
      if (fs.existsSync(fullPath)) {
        return createRequire(fullPath);
      }
    }
    return createRequire(path.join(projectPath, "index.js"));
  }
}
