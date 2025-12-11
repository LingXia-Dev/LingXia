import * as fs from 'fs';
import * as path from 'path';
import type { Page, PageFiles } from '../../types/index.js';

/**
 * Abstract base class for framework processors
 * Each framework implements this interface for specific handling
 */
export abstract class FrameworkProcessor {
  protected projectPath: string;
  protected outputDir: string;
  protected templatesDir: string;

  constructor(projectPath: string, outputDir: string, templatesDir: string) {
    this.projectPath = projectPath;
    this.outputDir = outputDir;
    this.templatesDir = templatesDir;
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
   * Copy framework templates to build directory
   */
  protected copyTemplates(buildDir: string): void {
    const frameworkTemplateDir = path.join(this.templatesDir, this.getFrameworkName().toLowerCase());

    if (!fs.existsSync(frameworkTemplateDir)) {
      throw new Error(`Framework templates not found: ${this.getFrameworkName()}`);
    }

    const templateFiles = fs.readdirSync(frameworkTemplateDir);
    for (const file of templateFiles) {
      const sourcePath = path.join(frameworkTemplateDir, file);
      const destPath = path.join(buildDir, file);

      if (fs.statSync(sourcePath).isFile()) {
        fs.copyFileSync(sourcePath, destPath);
      }
    }
  }

  protected normalizeAssetDir(dir?: string): string {
    const normalized = (dir ?? 'assets').replace(/^\/+/, '').replace(/\/+$/, '');
    return normalized.length > 0 ? normalized : 'assets';
  }
}
