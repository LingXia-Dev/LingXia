import fs from "fs";
import path from "path";
import type { Page, PageFiles, BuildOptions } from "../../types/index.js";
import { FileUtils } from "../utils/file.js";
import { PageProcessor } from "./page.js";
import { extractPageFunctionsFromSource } from "./page-functions.js";
import { extractPageTypes } from "./page-types.js";
import { validateViewFile } from "./view-validator.js";
import {
  DEFAULT_STATIC_DIRS,
  resolveStaticDirs,
} from "../constants/static-dirs.js";
import { readProjectFramework } from "../config/framework.js";
import type { ProjectFramework } from "../config/framework.js";
import type { BuildConfig } from "../config/lxapp-config.js";
import { TypeGenerator } from "../type-generator.js";

export class ViewBuilder {
  private projectPath: string;
  private outputDir: string;
  private fileUtils: FileUtils;
  private pageProcessor: PageProcessor;
  private staticDirs: string[];
  private framework: ProjectFramework;

  constructor(
    projectPath: string,
    outputDir: string,
    buildConfig?: BuildConfig,
    framework?: ProjectFramework,
  ) {
    this.projectPath = projectPath;
    this.outputDir = outputDir;
    this.fileUtils = new FileUtils();
    this.staticDirs = resolveStaticDirs(projectPath, buildConfig);
    // Use provided framework or auto-detect
    this.framework = framework ?? readProjectFramework(projectPath);
    this.pageProcessor = new PageProcessor(
      projectPath,
      outputDir,
      this.staticDirs,
      buildConfig,
      this.framework,
    );
  }

  async buildPages(pages: Page[], options: BuildOptions = {}): Promise<void> {
    await this.copyStaticAssets();

    // Group pages by framework
    const reactPages: Page[] = [];
    const vuePages: Page[] = [];
    const htmlPages: Page[] = [];
    for (const p of pages) {
      if (p.type === "react") reactPages.push(p);
      else if (p.type === "vue") vuePages.push(p);
      else if (p.type === "html") htmlPages.push(p);
    }

    // Pure HTML project mode: all pages must be HTML
    if (htmlPages.length > 0) {
      if (reactPages.length > 0 || vuePages.length > 0) {
        throw new Error(
          `Mixed HTML and React/Vue pages not supported. ` +
          `Project must be either all HTML or all React/Vue.`
        );
      }
      // Build HTML pages
      await this.copyRootFiles(htmlPages);
      await this.buildHtmlPages(htmlPages, options);
      return;
    }

    // Filter pages by framework - only build pages matching the configured framework
    const pagesToBuild = this.framework === "react" ? reactPages : vuePages;

    if (pagesToBuild.length === 0) {
      const otherFramework = this.framework === "react" ? "vue" : "react";
      const otherCount = this.framework === "react" ? vuePages.length : reactPages.length;
      if (otherCount > 0) {
        throw new Error(
          `No ${this.framework} pages found. Found ${otherCount} ${otherFramework} pages instead. ` +
          `Use --framework ${otherFramework} to build them.`,
        );
      }
      throw new Error(`No pages found for framework: ${this.framework}`);
    }

    // Copy root files with actual page paths (with extensions)
    await this.copyRootFiles(pagesToBuild);

    // Generate TypeScript type definitions for useLingXia
    await this.generatePageTypes(pagesToBuild);

    // Batch build pages using multi-entry
    const buildBatch = async (framework: "react" | "vue", subset: Page[]) => {
      if (subset.length === 0) return;
      const items = subset.map((page) => {
        const pageFiles = this.detectPageFiles(page);
        // Validate that view files don't use lx.* APIs (must use useLingXia() instead)
        validateViewFile(pageFiles);
        const pageFunctions = this.extractPageFunctions(pageFiles);
        return { page, pageFiles, pageFunctions };
      });

      // Always use Vite for view builds
      await this.pageProcessor.buildPagesBatch(framework, items, options);
    };

    // At this point, framework must be "react" or "vue" (HTML pages were handled above)
    if (this.framework !== "react" && this.framework !== "vue") {
      throw new Error(`Unexpected framework: ${this.framework}`);
    }
    await buildBatch(this.framework, pagesToBuild);
  }

  /**
   * Build pure HTML pages by copying HTML files and processing JS files with esbuild.
   */
  private async buildHtmlPages(pages: Page[], options: BuildOptions): Promise<void> {
    // Output directly to dist/ (same as React/Vue projects, no view/ subdirectory)
    const outputBase = path.join(this.projectPath, "dist");
    const libDir = path.join(this.projectPath, "lib");
    const target = options.target || "es2020";

    console.log(`📦 Building ${pages.length} HTML page(s) (target: ${target})`);

    // Copy lib/ directory if exists
    if (fs.existsSync(libDir)) {
      const libOutputDir = path.join(outputBase, "lib");
      await fs.promises.mkdir(libOutputDir, { recursive: true });
      const libFiles = fs.readdirSync(libDir);
      for (const file of libFiles) {
        const srcPath = path.join(libDir, file);
        const destPath = path.join(libOutputDir, file);
        if (fs.statSync(srcPath).isFile()) {
          // Process JS files with esbuild for target transpilation
          if (file.endsWith(".js")) {
            const esbuild = await import("esbuild");
            const result = await esbuild.build({
              entryPoints: [srcPath],
              outfile: destPath,
              bundle: false,
              minify: options.minify ?? true,
              target: target,
              format: "iife",
              write: true,
            });
            if (result.errors.length > 0) {
              throw new Error(`Failed to process ${file}: ${result.errors[0].text}`);
            }
          } else {
            await fs.promises.copyFile(srcPath, destPath);
          }
        }
      }
      console.log(`  ✓ Copied lib/ directory`);
    }

    // Process each HTML page
    for (const page of pages) {
      const pageDir = path.dirname(page.path);
      const baseName = path.basename(page.path, path.extname(page.path));
      const sourceDir = path.join(this.projectPath, pageDir);
      const outputDir = path.join(outputBase, pageDir);

      await fs.promises.mkdir(outputDir, { recursive: true });

      // Copy HTML file and inject runtime.js
      const htmlSrc = path.join(this.projectPath, page.path);
      const htmlDest = path.join(outputDir, `${baseName}.html`);
      let htmlContent = await fs.promises.readFile(htmlSrc, "utf-8");
      htmlContent = this.injectRuntimeScript(htmlContent);
      await fs.promises.writeFile(htmlDest, htmlContent, "utf-8");

      // Process accompanying JS file if exists
      const jsFileName = `${baseName}.js`;
      const jsSrc = path.join(sourceDir, jsFileName);
      if (fs.existsSync(jsSrc)) {
        const jsDest = path.join(outputDir, jsFileName);
        const esbuild = await import("esbuild");
        await esbuild.build({
          entryPoints: [jsSrc],
          outfile: jsDest,
          bundle: false,
          minify: options.minify ?? true,
          target: target,
          format: "iife",
          write: true,
        });
      }

      // Copy CSS file if exists
      const cssFileName = `${baseName}.css`;
      const cssSrc = path.join(sourceDir, cssFileName);
      if (fs.existsSync(cssSrc)) {
        const cssDest = path.join(outputDir, cssFileName);
        await fs.promises.copyFile(cssSrc, cssDest);
      }

      // Copy page config (index.json) if exists
      const jsonFileName = `${baseName}.json`;
      const jsonSrc = path.join(sourceDir, jsonFileName);
      if (fs.existsSync(jsonSrc)) {
        const jsonDest = path.join(outputDir, jsonFileName);
        await fs.promises.copyFile(jsonSrc, jsonDest);
      }

      console.log(`  ✓ ${page.path}`);
    }

    console.log(`✅ HTML view build complete`);
  }

  /**
   * Inject runtime.js script into HTML content for LingXiaBridge support.
   */
  private injectRuntimeScript(htmlContent: string): string {
    const runtimeSrc = "lx://assets/runtime.js";
    if (htmlContent.toLowerCase().includes(runtimeSrc)) {
      return htmlContent;
    }

    const scriptTag = `<script src="${runtimeSrc}"></script>`;
    const lower = htmlContent.toLowerCase();
    const headIndex = lower.indexOf("</head>");
    if (headIndex !== -1) {
      return `${htmlContent.slice(0, headIndex)}  ${scriptTag}\n${htmlContent.slice(headIndex)}`;
    }

    const bodyIndex = lower.indexOf("<body");
    if (bodyIndex !== -1) {
      const bodyEnd = htmlContent.indexOf(">", bodyIndex);
      if (bodyEnd !== -1) {
        const insertPos = bodyEnd + 1;
        return `${htmlContent.slice(0, insertPos)}\n  ${scriptTag}${htmlContent.slice(insertPos)}`;
      }
    }

    return `${scriptTag}\n${htmlContent}`;
  }

  /**
   * Generate TypeScript type definitions for each page's useLingXia hook.
   * Types are extracted from the Logic layer (index.ts) and written to .lingxia/types/
   */
  private async generatePageTypes(pages: Page[]): Promise<void> {
    const typeGenerator = new TypeGenerator(this.projectPath);
    const generatedPaths: string[] = [];

    for (const page of pages) {
      const pageFiles = this.detectPageFiles(page);
      if (!pageFiles.logic.exists || !pageFiles.logic.path) {
        continue;
      }

      try {
        const logicContent = fs.readFileSync(pageFiles.logic.path, "utf-8");
        const typeInfo = extractPageTypes(logicContent);

        // Only generate if we found data or methods
        if (Object.keys(typeInfo.data).length > 0 || Object.keys(typeInfo.methods).length > 0) {
          const typeContent = typeGenerator.generatePageTypes(page.path, typeInfo);
          typeGenerator.writeTypesForPage(page.path, typeContent);
          generatedPaths.push(page.path.replace(/\.(tsx?|vue)$/, ".d.ts"));
        }
      } catch (error) {
        console.warn(`⚠️ Failed to generate types for ${page.path}:`, error);
      }
    }

    if (generatedPaths.length > 0) {
      typeGenerator.writeTypesConfig();
      typeGenerator.generateIndexFile(generatedPaths);
      console.log(`📝 Generated types for ${generatedPaths.length} page(s) in .lingxia/types/`);
    }
  }

  private detectPageFiles(page: Page): PageFiles {
    const pageDir = path.dirname(page.path);
    const baseName = path.basename(page.path, path.extname(page.path));
    const sourcePageDir = path.join(this.projectPath, pageDir);

    // The view file is the page file itself (use actual path from lxapp.json)
    const viewPath = path.join(this.projectPath, page.path);
    const viewExists = fs.existsSync(viewPath);
    const logicPath = this.findLogicFile(sourcePageDir, baseName);

    return {
      view: {
        path: viewPath,
        exists: viewExists,
        type: page.type,
      },
      logic: {
        path: logicPath ?? undefined,
        exists: logicPath !== null,
      },
      config: {
        path: path.join(sourcePageDir, `${baseName}.json`),
        exists: fs.existsSync(path.join(sourcePageDir, `${baseName}.json`)),
      },
      style: {
        path: path.join(sourcePageDir, `${baseName}.css`),
        exists: fs.existsSync(path.join(sourcePageDir, `${baseName}.css`)),
      },
    };
  }

  /**
   * Find logic file (.ts or .js) for a page
   */
  private findLogicFile(
    sourcePageDir: string,
    baseName: string,
  ): string | null {
    const tsPath = path.join(sourcePageDir, `${baseName}.ts`);
    const jsPath = path.join(sourcePageDir, `${baseName}.js`);

    if (fs.existsSync(tsPath)) {
      return tsPath;
    } else if (fs.existsSync(jsPath)) {
      return jsPath;
    }

    return null;
  }

  private extractPageFunctions(pageFiles: PageFiles): string[] {
    if (!pageFiles.logic.exists || !pageFiles.logic.path) {
      return [];
    }

    try {
      const logicContent = fs.readFileSync(pageFiles.logic.path, "utf-8");
      return extractPageFunctionsFromSource(logicContent);
    } catch (error) {
      console.warn(
        `⚠️ Failed to extract functions from ${pageFiles.logic.path}`,
      );
      return [];
    }
  }

  private async copyStaticAssets(): Promise<void> {
    for (const dirName of this.staticDirs) {
      const sourceDir = path.join(this.projectPath, dirName);
      if (fs.existsSync(sourceDir)) {
        const destDir = path.join(this.outputDir, dirName);
        await this.fileUtils.copyDirectory(sourceDir, destDir);
      }
    }
  }

  private async copyRootFiles(pages: Page[]): Promise<void> {
    // Copy and update lxapp.json with actual page paths (with extensions)
    const lxappJson = path.join(this.projectPath, "lxapp.json");
    if (fs.existsSync(lxappJson)) {
      const content = JSON.parse(fs.readFileSync(lxappJson, "utf-8"));

      // Helper to strip extension from path
      const stripExt = (p: string) => {
        const ext = path.extname(p);
        return ext ? p.slice(0, -ext.length) : p;
      };

      // Build a map from page name (without extension) to actual path (with extension)
      const pagePathMap = new Map<string, string>();
      for (const page of pages) {
        pagePathMap.set(stripExt(page.path), page.path);
      }

      // Update pages array
      if (Array.isArray(content.pages)) {
        content.pages = content.pages.map((p: string) => pagePathMap.get(stripExt(p)) ?? p);
      }

      // Update tabBar.list[].pagePath
      if (content.tabBar?.list && Array.isArray(content.tabBar.list)) {
        content.tabBar.list = content.tabBar.list.map((item: { pagePath?: string; [key: string]: unknown }) => {
          if (item.pagePath) {
            const actualPath = pagePathMap.get(stripExt(item.pagePath));
            if (actualPath) return { ...item, pagePath: actualPath };
          }
          return item;
        });
      }

      const destFile = path.join(this.outputDir, "lxapp.json");
      fs.writeFileSync(destFile, JSON.stringify(content, null, 2));
    }

    // Process lxapp.css with import resolution
    const lxappCss = path.join(this.projectPath, "lxapp.css");
    if (fs.existsSync(lxappCss)) {
      await this.processLxappCss(lxappCss);
    }
  }

  private async processLxappCss(cssPath: string): Promise<void> {
    const finalCss = await this.resolveCssImports(cssPath, new Set());
    const destFile = path.join(this.outputDir, "lxapp.css");
    fs.writeFileSync(destFile, finalCss);
  }

  private async resolveCssImports(
    cssPath: string,
    processedFiles: Set<string>,
  ): Promise<string> {
    const absolutePath = path.resolve(cssPath);
    if (processedFiles.has(absolutePath)) {
      console.warn(`⚠️ Circular import detected: ${cssPath}`);
      return "";
    }
    processedFiles.add(absolutePath);

    if (!fs.existsSync(cssPath)) {
      console.warn(`⚠️ CSS file not found: ${cssPath}`);
      return "";
    }

    const cssContent = fs.readFileSync(cssPath, "utf-8");
    const cssDir = path.dirname(cssPath);
    let resolvedCss = "";

    const lines = cssContent.split("\n");
    for (const line of lines) {
      const trimmedLine = line.trim();

      const importMatch = trimmedLine.match(/^@import\s+['"]([^'"]+)['"];?/);
      if (importMatch) {
        const importPath = importMatch[1];
        let resolvedPath;

        if (importPath.startsWith("./") || importPath.startsWith("../")) {
          resolvedPath = path.resolve(cssDir, importPath);
        } else if (
          !importPath.startsWith("http") &&
          !importPath.startsWith("//")
        ) {
          resolvedPath = path.resolve(this.projectPath, importPath);
        } else {
          resolvedCss += line + "\n";
          continue;
        }

        const importedCss = await this.resolveCssImports(
          resolvedPath,
          processedFiles,
        );
        if (importedCss) {
          resolvedCss += `/* Imported from: ${importPath} */\n`;
          resolvedCss += importedCss + "\n";
        }
      } else {
        resolvedCss += line + "\n";
      }
    }

    return resolvedCss;
  }
}
