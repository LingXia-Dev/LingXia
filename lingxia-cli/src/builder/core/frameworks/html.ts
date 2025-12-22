import * as fs from 'fs';
import * as path from 'path';
import { FrameworkProcessor } from './base.js';
import type { Page, PageFiles } from '../../types/index.js';
import { getPageTitle } from '../utils/page.js';
import { FileUtils } from '../utils/file.js';

/**
 * HTML framework processor
 * Handles static HTML pages
 */
export class HtmlProcessor extends FrameworkProcessor {
  private fileUtils: FileUtils;

  constructor(projectPath: string, outputDir: string, templatesDir: string) {
    super(projectPath, outputDir, templatesDir);
    this.fileUtils = new FileUtils();
  }

  getFrameworkName(): string {
    return 'HTML';
  }

  getExtensions(): string[] {
    return ['.html'];
  }

  getDependencies(): { dependencies: any; devDependencies: any } {
    return {
      dependencies: {},
      devDependencies: {}
    };
  }

  async createViteConfig(buildDir: string, options: any = {}): Promise<any> {
    // HTML doesn't need Vite config
    return null;
  }

  async setupBuild(
    buildDir: string,
    page: Page,
    pageFiles: PageFiles,
    pageFunctions: string[]
  ): Promise<void> {
    // HTML pages don't need build setup
    // They are processed directly
  }

  async generateOutput(
    page: Page,
    pageFiles: PageFiles,
    buildResult: { distDir: string; assetDir?: string; entryHtml?: string; entryJs?: string },
    bridgeScript: string
  ): Promise<void> {
    const pageOutputDir = path.join(this.outputDir, path.dirname(page.path));
    const baseName = path.basename(page.path, path.extname(page.path));

    // Read and process HTML content
    let htmlContent = fs.readFileSync(pageFiles.view.path, 'utf-8');

    // Process page title
    const pageTitle = getPageTitle(page, pageFiles);
    htmlContent = this.processHtmlPageTitle(htmlContent, pageTitle);
    htmlContent = this.injectRuntimeScript(htmlContent);

    // Inject page function bridge
    htmlContent = htmlContent.replace(
      '</body>',
      `<script>\n${bridgeScript}\n</script>\n</body>`
    );

    // Ensure output directory exists
    this.fileUtils.ensureDirectory(pageOutputDir);

    // Copy page CSS if exists
    if (pageFiles.style.exists) {
      const cssOutputPath = path.join(pageOutputDir, `${baseName}.css`);
      fs.copyFileSync(pageFiles.style.path, cssOutputPath);
    }

    // Copy page config if exists
    if (pageFiles.config.exists) {
      const configOutputPath = path.join(pageOutputDir, `${baseName}.json`);
      fs.copyFileSync(pageFiles.config.path, configOutputPath);
    }

    // Write final HTML
    const htmlOutputPath = path.join(pageOutputDir, `${baseName}.html`);
    fs.writeFileSync(htmlOutputPath, htmlContent);
  }

  /**
   * Process HTML page title based on page configuration
   * Handles both cases: existing title and no title
   */
  private processHtmlPageTitle(htmlContent: string, pageTitle: string): string {
    // Check if HTML already has a title tag
    const titleRegex = /<title[^>]*>.*?<\/title>/i;
    const hasTitle = titleRegex.test(htmlContent);

    if (hasTitle) {
      // Replace existing title
      return htmlContent.replace(titleRegex, `<title>${pageTitle}</title>`);
    } else {
      // Add title to head section
      const headRegex = /<head[^>]*>/i;
      if (headRegex.test(htmlContent)) {
        return htmlContent.replace(headRegex, `$&\n    <title>${pageTitle}</title>`);
      } else {
        // Fallback: add after opening html tag
        const htmlRegex = /<html[^>]*>/i;
        if (htmlRegex.test(htmlContent)) {
          return htmlContent.replace(htmlRegex, `$&\n<head>\n    <title>${pageTitle}</title>\n</head>`);
        }
      }
    }

    return htmlContent;
  }


}
