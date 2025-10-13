import fs from 'fs';
import path from 'path';
import type { Page, PageFiles, BuildOptions } from '../../types/index.js';
import { FileUtils } from '../utils/file.js';
import { PageProcessor } from './page.js';

export class ViewBuilder {
  private projectPath: string;
  private outputDir: string;
  private fileUtils: FileUtils;
  private pageProcessor: PageProcessor;

  constructor(projectPath: string, outputDir: string) {
    this.projectPath = projectPath;
    this.outputDir = outputDir;
    this.fileUtils = new FileUtils();
    this.pageProcessor = new PageProcessor(projectPath, outputDir);
  }

  async buildPages(pages: Page[], options: BuildOptions = {}): Promise<void> {
    console.log(' Building pages...');

    await this.copyStaticAssets();
    await this.copyRootFiles();

    // Group pages by framework
    const htmlPages: Page[] = [];
    const reactPages: Page[] = [];
    const vuePages: Page[] = [];
    for (const p of pages) {
      if (p.type === 'react') reactPages.push(p);
      else if (p.type === 'vue') vuePages.push(p);
      else htmlPages.push(p);
    }

    // Build HTML pages individually (no bundler)
    for (const page of htmlPages) {
      await this.buildPage(page, options);
    }

    // Batch build React/Vue using multi-entry
    const buildBatch = async (framework: 'react' | 'vue', subset: Page[]) => {
      if (subset.length === 0) return;
      const items = subset.map(page => {
        const pageFiles = this.detectPageFiles(page);
        const pageFunctions = this.extractPageFunctions(pageFiles);
        return { page, pageFiles, pageFunctions };
      });

      // Always use Vite for view builds
      await this.pageProcessor.buildPagesBatch(framework, items, options);
    };

    // Parallel build for better performance
    const buildPromises = [];
    if (reactPages.length > 0) {
      buildPromises.push(buildBatch('react', reactPages));
    }
    if (vuePages.length > 0) {
      buildPromises.push(buildBatch('vue', vuePages));
    }
    
    await Promise.all(buildPromises);
  }

  private async buildPage(page: Page, options: BuildOptions = {}): Promise<void> {
    const pageFiles = this.detectPageFiles(page);

    // Validate page files exist
    if (!pageFiles.view.exists) {
      throw new Error(`View file not found for page: ${page.path}`);
    }

    const pageFunctions = this.extractPageFunctions(pageFiles);

    // Delegate to PageProcessor for actual building
    await this.pageProcessor.buildPage(page, pageFiles, pageFunctions, options);
  }

  private detectPageFiles(page: Page): PageFiles {
    const pageDir = path.dirname(page.path);
    const baseName = path.basename(page.path, path.extname(page.path));
    const sourcePageDir = path.join(this.projectPath, pageDir);

    // The view file is the page file itself (use actual path from lxapp.json)
    const viewPath = path.join(this.projectPath, page.path);
    const viewExists = fs.existsSync(viewPath);



    return {
      view: {
        path: viewPath,
        exists: viewExists,
        type: page.type
      },
      logic: {
        path: this.findLogicFile(sourcePageDir, baseName),
        exists: this.findLogicFile(sourcePageDir, baseName) !== null
      },
      config: {
        path: path.join(sourcePageDir, `${baseName}.json`),
        exists: fs.existsSync(path.join(sourcePageDir, `${baseName}.json`))
      },
      style: {
        path: path.join(sourcePageDir, `${baseName}.css`),
        exists: fs.existsSync(path.join(sourcePageDir, `${baseName}.css`))
      }
    };
  }

  /**
   * Find logic file (.ts or .js) for a page
   */
  private findLogicFile(sourcePageDir: string, baseName: string): string | null {
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
      const logicContent = fs.readFileSync(pageFiles.logic.path, 'utf-8');
      const functions: string[] = [];

      // Find Page({ ... }) call and extract functions from its parameter object
      const pageCallRegex = /Page\s*\(\s*\{([\s\S]*)\}\s*\)/;
      const pageMatch = pageCallRegex.exec(logicContent);

      if (pageMatch) {
        const pageObjectContent = pageMatch[1];

        // Extract function properties from the Page object
        // Matches: functionName: function() {}, functionName: async function() {}
        const functionPropertyRegex = /([a-zA-Z_$][a-zA-Z0-9_$]*)\s*:\s*(?:async\s+)?function/g;
        let match: RegExpExecArray | null;

        while ((match = functionPropertyRegex.exec(pageObjectContent)) !== null) {
          const functionName = match[1];
          if (functionName && !functionName.startsWith('_')) {
            functions.push(functionName);
          }
        }
      }

      // Filter out lifecycle functions - these are handled by the runtime
      const lifecycleFunctions = [
        'onLoad', 'onShow', 'onHide', 'onUnload', 'onReady',
      ];

      const bridgeFunctions = functions.filter(func => !lifecycleFunctions.includes(func));

      return [...new Set(bridgeFunctions)]; // Remove duplicates
    } catch (error) {
      console.warn(`⚠️ Failed to extract functions from ${pageFiles.logic.path}`);
      return [];
    }
  }

  private async copyStaticAssets(): Promise<void> {
    const staticDirs = ['images', 'assets', 'static'];

    for (const dirName of staticDirs) {
      const sourceDir = path.join(this.projectPath, dirName);
      if (fs.existsSync(sourceDir)) {
        const destDir = path.join(this.outputDir, dirName);
        await this.fileUtils.copyDirectory(sourceDir, destDir);
        console.log(` Copied ${dirName} to dist`);
      }
    }
  }

  private async copyRootFiles(): Promise<void> {
    // Copy lxapp.json
    const lxappJson = path.join(this.projectPath, 'lxapp.json');
    if (fs.existsSync(lxappJson)) {
      const destFile = path.join(this.outputDir, 'lxapp.json');
      fs.copyFileSync(lxappJson, destFile);
      console.log(` Copied lxapp.json to dist`);
    }

    // Process lxapp.css with import resolution
    const lxappCss = path.join(this.projectPath, 'lxapp.css');
    if (fs.existsSync(lxappCss)) {
      await this.processLxappCss(lxappCss);
    }
  }

  private async processLxappCss(cssPath: string): Promise<void> {
    console.log(` Processing lxapp.css with import resolution...`);

    const finalCss = await this.resolveCssImports(cssPath, new Set());
    const destFile = path.join(this.outputDir, 'lxapp.css');
    fs.writeFileSync(destFile, finalCss);

    console.log(` Generated final lxapp.css to dist`);
  }

  private async resolveCssImports(cssPath: string, processedFiles: Set<string>): Promise<string> {
    const absolutePath = path.resolve(cssPath);
    if (processedFiles.has(absolutePath)) {
      console.warn(`⚠️ Circular import detected: ${cssPath}`);
      return '';
    }
    processedFiles.add(absolutePath);

    if (!fs.existsSync(cssPath)) {
      console.warn(`⚠️ CSS file not found: ${cssPath}`);
      return '';
    }

    const cssContent = fs.readFileSync(cssPath, 'utf-8');
    const cssDir = path.dirname(cssPath);
    let resolvedCss = '';

    const lines = cssContent.split('\n');
    for (const line of lines) {
      const trimmedLine = line.trim();

      const importMatch = trimmedLine.match(/^@import\s+['"]([^'"]+)['"];?/);
      if (importMatch) {
        const importPath = importMatch[1];
        let resolvedPath;

        if (importPath.startsWith('./') || importPath.startsWith('../')) {
          resolvedPath = path.resolve(cssDir, importPath);
        } else if (!importPath.startsWith('http') && !importPath.startsWith('//')) {
          resolvedPath = path.resolve(this.projectPath, importPath);
        } else {
          resolvedCss += line + '\n';
          continue;
        }

        const importedCss = await this.resolveCssImports(resolvedPath, processedFiles);
        if (importedCss) {
          resolvedCss += `/* Imported from: ${importPath} */\n`;
          resolvedCss += importedCss + '\n';
        }
      } else {
        resolvedCss += line + '\n';
      }
    }

    return resolvedCss;
  }
}
