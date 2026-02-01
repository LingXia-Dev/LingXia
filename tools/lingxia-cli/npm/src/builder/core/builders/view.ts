import fs from "fs";
import path from "path";
import type { Page, PageFiles, BuildOptions } from "../../types/index.js";
import { FileUtils } from "../utils/file.js";
import { PageProcessor } from "./page.js";
import { extractPageFunctionsFromSource } from "./page-functions.js";
import { validateViewFile } from "./view-validator.js";
import {
  DEFAULT_STATIC_DIRS,
  resolveStaticDirs,
} from "../constants/static-dirs.js";
import { readProjectFramework } from "../config/framework.js";
import type { ProjectFramework } from "../config/framework.js";
import type { BuildConfig } from "../config/lxapp-config.js";

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

    // Group pages by framework and validate supported types
    const reactPages: Page[] = [];
    const vuePages: Page[] = [];
    const unsupported: Page[] = [];
    for (const p of pages) {
      if (p.type === "react") reactPages.push(p);
      else if (p.type === "vue") vuePages.push(p);
      else unsupported.push(p);
    }
    if (unsupported.length > 0) {
      const paths = unsupported.map((p) => p.path).join(", ");
      throw new Error(
        `HTML pages are no longer supported. Please migrate these entries to React/Vue: ${paths}`,
      );
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

    await buildBatch(this.framework, pagesToBuild);
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
