import fs from "fs";
import path from "path";
import { FileUtils } from "../utils/file.js";
import { ConfigManager } from "../config.js";
import type { BuildOptions } from "../../types/index.js";
import { injectPagePath } from "./page-path-injector.js";
import {
  DEFAULT_SOURCE_DIRS,
  resolveSourceDirs,
} from "../constants/source-dirs.js";
import { resolveAliasMap } from "../config/alias-config.js";
import type { BuildConfig } from "../config/lxapp-config.js";

/**
 * Modern LogicBuilder that leverages Vite for dependency resolution and bundling
 */
export class LogicBuilder {
  private projectPath: string;
  private outputDir: string;
  private fileUtils: FileUtils;
  private configManager: ConfigManager;
  private alias: Record<string, string>;
  private sourceDirs: string[];
  private isPlugin: boolean;
  private pluginId?: string;

  constructor(
    projectPath: string,
    outputDir: string,
    pluginId?: string,
    buildConfig?: BuildConfig,
  ) {
    this.projectPath = projectPath;
    this.outputDir = outputDir;
    this.fileUtils = new FileUtils();
    this.configManager = new ConfigManager(projectPath);
    this.alias = resolveAliasMap(projectPath, buildConfig);
    this.sourceDirs =
      resolveSourceDirs(projectPath, buildConfig) ?? DEFAULT_SOURCE_DIRS;
    this.isPlugin = pluginId !== undefined;
    this.pluginId = pluginId;
  }

  async buildLogic(
    options: BuildOptions = {},
    resolvedPages?: string[],
  ): Promise<void> {
    // Use resolved pages (with extensions) if provided, otherwise get from config
    const pages =
      resolvedPages ?? this.configManager.getPages({ plugin: this.isPlugin });
    const logicFiles = this.discoverLogicFiles(pages);

    if (logicFiles.length === 0) {
      return;
    }

    // Use Vite to build logic layer with proper dependency resolution
    await this.buildLogicWithVite(logicFiles, pages, options);
  }

  /**
   * Discover logic files based on pages configuration
   */
  private discoverLogicFiles(pages: string[]): string[] {
    const logicFiles: string[] = [];

    // Add lxapp.js or lxapp.ts if it exists
    const lxappJsPath = path.join(this.projectPath, "lxapp.js");
    const lxappTsPath = path.join(this.projectPath, "lxapp.ts");

    if (fs.existsSync(lxappTsPath)) {
      logicFiles.push(lxappTsPath);
    } else if (fs.existsSync(lxappJsPath)) {
      logicFiles.push(lxappJsPath);
    }

    // Process each page path
    for (const pagePath of pages) {
      // Remove extension and try .js and .ts
      const basePath = path.join(
        this.projectPath,
        pagePath.replace(/\.[^.]+$/, ""),
      );

      // Check which logic file exists
      const jsPath = `${basePath}.js`;
      const tsPath = `${basePath}.ts`;

      if (fs.existsSync(jsPath)) {
        logicFiles.push(jsPath);
      } else if (fs.existsSync(tsPath)) {
        logicFiles.push(tsPath);
      }
    }

    return logicFiles;
  }

  /**
   * Build logic layer using Vite for proper dependency resolution
   */
  private async buildLogicWithVite(
    logicFiles: string[],
    pages: string[],
    options: BuildOptions = {},
  ): Promise<void> {
    const buildDir = path.join(this.projectPath, ".lingxia", "build", "logic");

    // Always clean build directory
    this.fileUtils.cleanDirectory(buildDir);
    await this.copySourceDirectories(buildDir);

    // Create entry file that imports all logic files
    const entryContent = this.createLogicEntry(logicFiles, pages);
    fs.writeFileSync(path.join(buildDir, "main.js"), entryContent);

    // Copy lxapp file so entry import resolves correctly
    this.copyLxappFile(logicFiles, buildDir);

    // Build with bundled Vite
    await this.runViteLogicBuild(buildDir, options);

    // Copy built logic.js to output
    const builtLogicPath = path.join(buildDir, "dist", "main.iife.js");
    const outputPath = path.join(this.outputDir, "logic.js");
    fs.copyFileSync(builtLogicPath, outputPath);
  }

  /**
   * Create entry file that imports all logic files
   */
  private createLogicEntry(logicFiles: string[], pages: string[]): string {
    const imports: string[] = [];

    for (let i = 0; i < logicFiles.length; i++) {
      const logicFile = logicFiles[i];
      const fileName = path.basename(logicFile);

      // Process the logic file to add path parameter to Page calls
      if (fileName !== "lxapp.js" && fileName !== "lxapp.ts") {
        const pagePath = this.getPagePathFromConfig(logicFile, pages);
        const importPath = this.processLogicFileForPath(logicFile, pagePath);
        imports.push(`import './${importPath}';`);
      } else {
        // For lxapp files, import as-is
        const relativePath = `./${fileName}`;
        imports.push(`import '${relativePath}';`);
      }
    }

    return imports.join("\n");
  }

  /**
   * Process logic file to add path parameter to Page calls
   */
  private processLogicFileForPath(
    sourceFile: string,
    pagePath: string,
  ): string {
    const buildDir = path.join(this.projectPath, ".lingxia", "build", "logic");
    const relativeDir = path.dirname(
      path.relative(this.projectPath, sourceFile),
    );
    const destDir =
      relativeDir && relativeDir !== "."
        ? path.join(buildDir, relativeDir)
        : buildDir;
    this.fileUtils.ensureDirectory(destDir);

    const ext = path.extname(sourceFile);
    const targetBase = `${path.basename(sourceFile, ext)}_processed${ext}`;
    const targetPath = path.join(destDir, targetBase);

    const content = fs.readFileSync(sourceFile, "utf-8");
    const transformedContent = injectPagePath(content, pagePath, {
      pluginId: this.pluginId,
    });
    fs.writeFileSync(targetPath, transformedContent);
    const posixDir =
      relativeDir && relativeDir !== "."
        ? relativeDir.split(path.sep).join("/")
        : "";
    return posixDir ? `${posixDir}/${targetBase}` : targetBase;
  }

  /**
   * Get page path from pages configuration
   */
  private getPagePathFromConfig(
    logicFilePath: string,
    pages: string[],
  ): string {
    // Extract the directory and filename from the logic file path
    const relativePath = path.relative(this.projectPath, logicFilePath);
    const logicDir = path.dirname(relativePath);
    const logicBaseName = path.basename(
      logicFilePath,
      path.extname(logicFilePath),
    );

    // Find the page path that corresponds to this logic file
    for (const pagePath of pages) {
      const pageDir = path.dirname(pagePath);
      const pageBaseName = path.basename(pagePath, path.extname(pagePath));

      // Check if this logic file corresponds to this page
      // Both directory and base name should match
      if (pageDir === logicDir && pageBaseName === logicBaseName) {
        return pagePath;
      }
    }

    // Fallback - this shouldn't happen if lxapp.json is correct
    return `${logicBaseName}.html`;
  }

  /**
   * Create Vite config for logic build using TemplateManager
   */
  /**
   * Copy lxapp.js/ts to build directory so entry import works.
   * Other page files are referenced through their processed copies.
   */
  private copyLxappFile(logicFiles: string[], buildDir: string): void {
    const lxappFile = logicFiles.find((file) => {
      const base = path.basename(file).toLowerCase();
      return base === "lxapp.js" || base === "lxapp.ts";
    });

    if (!lxappFile) {
      return;
    }

    const fileName = path.basename(lxappFile);
    const destPath = path.join(buildDir, fileName);
    fs.copyFileSync(lxappFile, destPath);
  }

  private async copySourceDirectories(buildDir: string): Promise<void> {
    for (const dir of this.sourceDirs) {
      const srcDir = path.join(this.projectPath, dir);
      if (fs.existsSync(srcDir)) {
        const destDir = path.join(buildDir, dir);
        await this.fileUtils.copyDirectory(srcDir, destDir);
      }
    }
  }

  private async runViteLogicBuild(
    buildDir: string,
    options: BuildOptions = {},
  ): Promise<void> {
    const { build } = await import("vite");
    const isProd = Boolean(options.release);
    const isDev = !isProd;

    await build({
      configFile: false,
      root: buildDir,
      logLevel: "warn",
      mode: isDev ? "development" : isProd ? "production" : undefined,
      resolve: {
        alias: this.alias,
      },
      build: {
        lib: {
          entry: path.join(buildDir, "main.js"),
          name: "LingXiaLogic",
          fileName: "main",
          formats: ["iife"],
        },
        outDir: path.join(buildDir, "dist"),
        emptyOutDir: true,
        minify: isProd ? "esbuild" : false,
        sourcemap: isDev,
      },
    });
  }
}
