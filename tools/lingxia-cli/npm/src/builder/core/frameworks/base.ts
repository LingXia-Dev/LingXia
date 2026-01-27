import * as fs from 'fs';
import * as path from 'path';
import type { Page, PageFiles } from '../../types/index.js';
import { getFrameworkTemplates } from './templates.js';

/**
 * Abstract base class for framework processors
 * Each framework implements this interface for specific handling
 */
export abstract class FrameworkProcessor {
  protected projectPath: string;
  protected outputDir: string;

  constructor(projectPath: string, outputDir: string, _templatesDir?: string) {
    this.projectPath = projectPath;
    this.outputDir = outputDir;
  }

  /**
   * Get framework name
   */
  abstract getFrameworkName(): string;

  /**
   * Get framework-specific file extensions
   */
  abstract getExtensions(): string[];

  /**
   * Setup framework-specific build environment
   */
  abstract setupBuild(
    buildDir: string,
    page: Page,
    pageFiles: PageFiles,
    pageFunctions: string[]
  ): Promise<void>;

  /**
   * Generate final output for this framework
   */
  abstract generateOutput(
    page: Page,
    pageFiles: PageFiles,
    buildResult: { distDir: string; assetDir?: string; entryHtml?: string; entryJs?: string },
    bridgeScript: string
  ): Promise<void>;

  /**
   * Get package.json dependencies for this framework
   */
  abstract getDependencies(): { dependencies: any; devDependencies: any };

  /**
   * Process page title in framework-specific way
   */
  protected processPageTitle(content: string, pageTitle: string): string {
    // Default implementation - can be overridden
    const titlePattern = new RegExp(`<title>LingXia ${this.getFrameworkName()} Page</title>`, 'i');
    return content.replace(titlePattern, `<title>${pageTitle}</title>`);
  }

  /**
   * Copy framework templates to build directory (uses embedded templates)
   */
  protected copyTemplates(buildDir: string): void {
    const frameworkName = this.getFrameworkName().toLowerCase();
    const templates = getFrameworkTemplates(frameworkName);

    if (!templates) {
      throw new Error(`Framework templates not found: ${this.getFrameworkName()}`);
    }

    fs.writeFileSync(path.join(buildDir, 'index.html'), templates.indexHtml);
    fs.writeFileSync(path.join(buildDir, templates.mainEntryFilename), templates.mainEntry);
  }

  protected normalizeAssetDir(dir?: string): string {
    const normalized = (dir ?? 'assets').replace(/^\/+/, '').replace(/\/+$/, '');
    return normalized.length > 0 ? normalized : 'assets';
  }

  protected injectRuntimeScript(htmlContent: string): string {
    const runtimeSrc = 'lx://assets/runtime.js';
    if (htmlContent.toLowerCase().includes(runtimeSrc)) {
      return htmlContent;
    }

    const scriptTag = `<script src="${runtimeSrc}"></script>`;
    const lower = htmlContent.toLowerCase();
    const headIndex = lower.indexOf('</head>');
    if (headIndex !== -1) {
      return `${htmlContent.slice(0, headIndex)}${scriptTag}\n${htmlContent.slice(headIndex)}`;
    }

    const bodyIndex = lower.indexOf('<body');
    if (bodyIndex !== -1) {
      const bodyEnd = htmlContent.indexOf('>', bodyIndex);
      if (bodyEnd !== -1) {
        const insertPos = bodyEnd + 1;
        return `${htmlContent.slice(0, insertPos)}${scriptTag}\n${htmlContent.slice(insertPos)}`;
      }
    }

    return `${scriptTag}\n${htmlContent}`;
  }
}
