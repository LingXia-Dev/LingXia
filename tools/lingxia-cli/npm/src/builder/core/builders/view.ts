import fs from "fs";
import path from "path";
import type { Page, PageFiles, BuildOptions } from "../../types/index.js";
import { FileUtils } from "../utils/file.js";
import { PageProcessor } from "./page.js";
import { extractPageTypes } from "./page-types.js";
import { validateViewFile } from "./view-validator.js";
import {
  DEFAULT_STATIC_DIRS,
  resolveStaticDirs,
} from "../constants/static-dirs.js";
import { TemplateManager, type PageBridgeMethod } from "../template.js";
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
  private templateManager: TemplateManager;

  constructor(
    projectPath: string,
    outputDir: string,
    buildConfig?: BuildConfig,
    framework?: ProjectFramework,
  ) {
    this.projectPath = projectPath;
    this.outputDir = outputDir;
    this.fileUtils = new FileUtils();
    this.templateManager = new TemplateManager();
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
        const pageFunctions = this.extractPageBridgeMethods(pageFiles);
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

    // Process view/ and lib/ directories with TS/JS compilation
    const scriptDirs = ["view", "lib"];
    for (const dirName of scriptDirs) {
      const srcDir = path.join(this.projectPath, dirName);
      if (!fs.existsSync(srcDir)) continue;
      
      const outDir = path.join(outputBase, dirName);
      await fs.promises.mkdir(outDir, { recursive: true });
      const files = fs.readdirSync(srcDir);
      
      for (const file of files) {
        const srcPath = path.join(srcDir, file);
        if (!fs.statSync(srcPath).isFile()) continue;
        
        // Compile TS/JS files with esbuild
        if (file.endsWith(".ts") || file.endsWith(".js")) {
          const outFile = file.replace(/\.ts$/, ".js");
          const destPath = path.join(outDir, outFile);
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
          // Copy other files as-is
          await fs.promises.copyFile(srcPath, path.join(outDir, file));
        }
      }
      console.log(`  ✓ Processed ${dirName}/ directory`);
    }

    // Process each HTML page
    for (const page of pages) {
      const pageDir = path.dirname(page.path);
      const baseName = path.basename(page.path, path.extname(page.path));
      const sourceDir = path.join(this.projectPath, pageDir);
      const outputDir = path.join(outputBase, pageDir);

      await fs.promises.mkdir(outputDir, { recursive: true });

      // Extract page functions from Logic layer
      const pageFunctions = this.extractHtmlBridgeMethods(sourceDir, baseName);

      // Copy HTML file and inject runtime.js + page function bridge
      const htmlSrc = path.join(this.projectPath, page.path);
      const htmlDest = path.join(outputDir, `${baseName}.html`);
      let htmlContent = await fs.promises.readFile(htmlSrc, "utf-8");
      htmlContent = this.injectRuntimeScript(htmlContent);
      htmlContent = this.injectPageFunctionBridge(htmlContent, pageFunctions);

      // Validate HTML script references before writing
      this.validateHtmlScriptReferences(htmlContent, page.path, pageDir, outputBase);

      await fs.promises.writeFile(htmlDest, htmlContent, "utf-8");

      // Process all TS/JS files in page directory (except Logic layer)
      const pageFiles = fs.readdirSync(sourceDir);
      for (const file of pageFiles) {
        const filePath = path.join(sourceDir, file);
        if (!fs.statSync(filePath).isFile()) continue;

        // Skip Logic layer files (index.ts/index.js)
        if (file === "index.ts" || file === "index.js") {
          continue;
        }

        // Compile TS/JS files with esbuild
        if (file.endsWith(".ts") || file.endsWith(".js")) {
          const outFile = file.replace(/\.ts$/, ".js");
          const destPath = path.join(outputDir, outFile);
          const esbuild = await import("esbuild");
          await esbuild.build({
            entryPoints: [filePath],
            outfile: destPath,
            bundle: false,
            minify: options.minify ?? true,
            target: target,
            format: "iife",
            write: true,
          });
        }
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

      if (pageFunctions.length > 0) {
        console.log(`  ✓ ${page.path} (${pageFunctions.length} functions)`);
      } else {
        console.log(`  ✓ ${page.path}`);
      }
    }

    console.log(`✅ HTML view build complete`);
  }

  /**
   * Extract page functions from the Logic layer for HTML pages.
   */
  private extractHtmlBridgeMethods(sourceDir: string, baseName: string): PageBridgeMethod[] {
    const tsPath = path.join(sourceDir, `${baseName}.ts`);
    const jsPath = path.join(sourceDir, `${baseName}.js`);
    const logicPath = fs.existsSync(tsPath) ? tsPath : fs.existsSync(jsPath) ? jsPath : null;

    if (!logicPath) {
      return [];
    }

    try {
      const logicContent = fs.readFileSync(logicPath, "utf-8");
      return this.templateManager.inferBridgeMethods(extractPageTypes(logicContent).methods);
    } catch (error) {
      console.warn(`⚠️ Failed to extract functions from ${logicPath}`);
      return [];
    }
  }

  /**
   * Inject page function bridge script into HTML content.
   */
  private injectPageFunctionBridge(htmlContent: string, functions: PageBridgeMethod[]): string {
    if (functions.length === 0) {
      return htmlContent;
    }

    const bridgeCode = this.templateManager.generateFunctionBridge(functions);
    const bridgeScript = `<script>${bridgeCode}</script>`;

    // Insert after runtime.js script
    const runtimePattern = /<script[^>]+src=["']lx:\/\/assets\/runtime\.js["'][^>]*><\/script>/i;
    const match = htmlContent.match(runtimePattern);
    if (match) {
      const insertPos = match.index! + match[0].length;
      return `${htmlContent.slice(0, insertPos)}\n  ${bridgeScript}${htmlContent.slice(insertPos)}`;
    }

    // Fallback: insert before </head>
    const headIndex = htmlContent.toLowerCase().indexOf("</head>");
    if (headIndex !== -1) {
      return `${htmlContent.slice(0, headIndex)}  ${bridgeScript}\n${htmlContent.slice(headIndex)}`;
    }

    // Last resort: prepend
    return `${bridgeScript}\n${htmlContent}`;
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
   * Validate script references in HTML files to enforce constraints.
   *
   * Allowed references:
   * - /view/*.js (shared components)
   * - /lib/*.js (libraries)
   * - <filename>.js in same directory (page-local scripts, but NOT index.js)
   *
   * Forbidden:
   * - index.js (Logic layer, not for client-side)
   * - Relative paths like ../
   * - Cross-page references like /pages/other/file.js
   */
  private validateHtmlScriptReferences(
    htmlContent: string,
    pagePath: string,
    pageDir: string,
    outputBase: string
  ): void {
    const scriptSrcRegex = /<script[^>]+src=["']([^"']+)["']/gi;
    const matches = [...htmlContent.matchAll(scriptSrcRegex)];

    for (const match of matches) {
      const src = match[1];

      // Skip runtime.js (auto-injected) and external URLs
      if (src === "lx://assets/runtime.js" || src.startsWith("http://") || src.startsWith("https://")) {
        continue;
      }

      // Check forbidden patterns
      if (src === "index.js") {
        throw new Error(
          `Invalid script reference in ${pagePath}:\n` +
          `  <script src="index.js">\n\n` +
          `index.js is reserved for the Logic layer (Rust-side).\n` +
          `For client-side scripts, use a different name:\n` +
          `  - view.js, client.js, home.js, etc.\n` +
          `  - /view/components.js (shared components)\n` +
          `  - /lib/utils.js (libraries)`
        );
      }

      if (src.includes("..")) {
        throw new Error(
          `Invalid script reference in ${pagePath}:\n` +
          `  <script src="${src}">\n\n` +
          `Relative paths with ".." are not allowed.\n` +
          `Use absolute paths: /view/*.js or /lib/*.js`
        );
      }

      if (src.startsWith("/pages/")) {
        throw new Error(
          `Invalid script reference in ${pagePath}:\n` +
          `  <script src="${src}">\n\n` +
          `Cross-page references are not allowed.\n` +
          `Move shared code to /view/ or /lib/ directories.`
        );
      }

      // Validate allowed patterns
      const isViewScript = /^\/view\/.+\.js$/.test(src);
      const isLibScript = /^\/lib\/.+\.js$/.test(src);
      const isLocalScript = /^[^/]+\.js$/.test(src); // Same directory, no path separator

      if (!isViewScript && !isLibScript && !isLocalScript) {
        throw new Error(
          `Invalid script reference in ${pagePath}:\n` +
          `  <script src="${src}">\n\n` +
          `HTML pages can only reference:\n` +
          `  - /view/*.js (shared components)\n` +
          `  - /lib/*.js (libraries)\n` +
          `  - <name>.js (page-local scripts in same directory)\n\n` +
          `Example:\n` +
          `  <script src="/view/components.js"></script>\n` +
          `  <script src="/lib/utils.js"></script>\n` +
          `  <script src="view.js"></script>`
        );
      }

      // Check source file exists (will be compiled to .js)
      if (isViewScript || isLibScript) {
        // /view/xxx.js or /lib/xxx.js -> check view/xxx.ts or view/xxx.js
        const relativePath = src.slice(1); // Remove leading /
        const tsSource = path.join(this.projectPath, relativePath.replace(/\.js$/, ".ts"));
        const jsSource = path.join(this.projectPath, relativePath);

        if (!fs.existsSync(tsSource) && !fs.existsSync(jsSource)) {
          throw new Error(
            `Missing script source in ${pagePath}:\n` +
            `  <script src="${src}">\n\n` +
            `Expected source file:\n` +
            `  ${tsSource} (or ${jsSource})\n\n` +
            `Create the file or remove the script tag.`
          );
        }
      } else if (isLocalScript) {
        // Page-local script -> check pages/xxx/yyy.ts or pages/xxx/yyy.js
        const sourceDir = path.join(this.projectPath, pageDir);
        const tsSource = path.join(sourceDir, src.replace(/\.js$/, ".ts"));
        const jsSource = path.join(sourceDir, src);

        if (!fs.existsSync(tsSource) && !fs.existsSync(jsSource)) {
          throw new Error(
            `Missing script source in ${pagePath}:\n` +
            `  <script src="${src}">\n\n` +
            `Expected source file:\n` +
            `  ${tsSource} (or ${jsSource})\n\n` +
            `Create the file or remove the script tag.`
          );
        }
      }
    }
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

  private extractPageBridgeMethods(pageFiles: PageFiles): PageBridgeMethod[] {
    if (!pageFiles.logic.exists || !pageFiles.logic.path) {
      return [];
    }

    try {
      const logicContent = fs.readFileSync(pageFiles.logic.path, "utf-8");
      return this.templateManager.inferBridgeMethods(extractPageTypes(logicContent).methods);
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
