import * as fs from 'fs';
import * as path from 'path';
import { load } from 'cheerio';
import { FrameworkProcessor } from './base.js';
import type { Page, PageFiles } from '../../types/index.js';
import { getPageTitle } from '../utils/page.js';
import { FileUtils } from '../utils/file.js';
import { TemplateManager } from '../template.js';

/**
 * React framework processor
 * Uses templates and framework-specific logic
 */
export class ReactProcessor extends FrameworkProcessor {
  private fileUtils: FileUtils;
  private templateManager: TemplateManager;

  constructor(projectPath: string, outputDir: string, templatesDir: string) {
    super(projectPath, outputDir, templatesDir);
    this.fileUtils = new FileUtils();
    this.templateManager = new TemplateManager();
  }

  getFrameworkName(): string {
    return 'React';
  }

  getExtensions(): string[] {
    return ['.tsx', '.jsx'];
  }

  getDependencies(): { dependencies: any; devDependencies: any } {
    return { dependencies: {}, devDependencies: {} };
  }

  async setupBuild(
    buildDir: string,
    page: Page,
    pageFiles: PageFiles,
    pageFunctions: string[]
  ): Promise<void> {
    // Copy framework templates
    this.copyTemplates(buildDir);

    // Process templates with page-specific data
    await this.processTemplates(buildDir, page, pageFiles, pageFunctions);
  }

  async generateOutput(
    page: Page,
    pageFiles: PageFiles,
    buildResult: { distDir: string; assetDir?: string; entryHtml?: string; entryJs?: string },
    bridgeScript: string
  ): Promise<void> {
    const pageOutputDir = path.join(this.outputDir, path.dirname(page.path));
    const baseName = path.basename(page.path, path.extname(page.path));

    const assetDir = this.normalizeAssetDir(buildResult.assetDir);

    // Copy built assets
    const builtIndexHtml = buildResult.entryHtml 
      ? path.join(buildResult.distDir, buildResult.entryHtml)
      : path.join(buildResult.distDir, 'index.html');
      
    const builtMainJs = buildResult.entryJs 
      ? path.join(buildResult.distDir, buildResult.entryJs)
      : path.join(buildResult.distDir, 'main.js');

    let htmlContent = fs.readFileSync(builtIndexHtml, 'utf-8');

    // Ensure output directory exists
    this.fileUtils.ensureDirectory(pageOutputDir);

    // Process JS
    if (fs.existsSync(builtMainJs)) {
      fs.copyFileSync(builtMainJs, path.join(pageOutputDir, 'view.js'));
    }

    // Copy page config
    if (pageFiles.config) {
      const configOutputPath = path.join(pageOutputDir, `${baseName}.json`);
      fs.copyFileSync(pageFiles.config.path, configOutputPath);
    }

    // Fix HTML paths and inject bridge script
    const assetRelativePath = path
      .relative(pageOutputDir, path.join(this.outputDir, assetDir))
      .split(path.sep)
      .join('/');
    htmlContent = this.fixHtmlPaths(htmlContent, baseName, assetDir, assetRelativePath);
    htmlContent = this.injectRuntimeScript(htmlContent);
    htmlContent = htmlContent.replace(
      '</body>',
      `<script>\n${bridgeScript}\n</script>\n</body>`
    );

    // Write final component file
    const componentOutputPath = path.join(pageOutputDir, `${baseName}.tsx`);
    fs.writeFileSync(componentOutputPath, htmlContent);
  }



  private async processTemplates(
    buildDir: string,
    page: Page,
    pageFiles: PageFiles,
    pageFunctions: string[]
  ): Promise<void> {
    const pageTitle = getPageTitle(page, pageFiles);

    // Process index.html
    const indexHtmlPath = path.join(buildDir, 'index.html');
    if (fs.existsSync(indexHtmlPath)) {
      let indexHtml = fs.readFileSync(indexHtmlPath, 'utf-8');
      indexHtml = this.processPageTitle(indexHtml, pageTitle);
      fs.writeFileSync(indexHtmlPath, indexHtml);
    }

    // Process main.jsx
    const mainJsxPath = path.join(buildDir, 'main.jsx');
    if (fs.existsSync(mainJsxPath)) {
      let mainJsx = fs.readFileSync(mainJsxPath, 'utf-8');

      // Inject page functions
      const bridgeScript = this.templateManager.generateFunctionBridge(pageFunctions);
      mainJsx = mainJsx.replace('/* {{PAGE_FUNCTIONS}} */', bridgeScript);

      const appImport = `import App from '${this.resolveSourceImportPath(buildDir, pageFiles.view.path)}';`;
      if (mainJsx.includes('/* {{APP_IMPORT}} */')) {
        mainJsx = mainJsx.replace('/* {{APP_IMPORT}} */', appImport);
      }

      fs.writeFileSync(mainJsxPath, mainJsx);
    }
  }

  /**
   * Fix HTML paths to use relative paths for React
   */
  private fixHtmlPaths(
    htmlContent: string,
    baseName: string,
    assetDir: string,
    assetRelativePath: string
  ): string {
    const normalizedAssetRelativePath =
      assetRelativePath.length === 0 ? '.' : assetRelativePath.replace(/\/+$/, '');
    const escapedDir = assetDir.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');

    const $ = load(htmlContent);

    $('script[src]').each((_, element) => {
      const $element = $(element);
      const src = $element.attr('src');
      if (!src) return;
      if (/^\/pages\/[^/]+\/[^/]+\.js(?:[?#].*)?$/i.test(src)) {
        $element.attr('src', './view.js');
      }
    });

    const buildAssetHref = (file: string, suffix: string) => {
      const basePath =
        normalizedAssetRelativePath === '.' ? `./${file}` : `${normalizedAssetRelativePath}/${file}`;
      return `${basePath}${suffix}`;
    };

    const rewriteAssetHref = (href: string): string | null => {
      if (!href) return null;
      let normalized = href.trim();
      if (normalized.startsWith('./')) {
        normalized = normalized.slice(2);
      }
      const assetPattern = new RegExp(`^/?${escapedDir}/([^?#]+)([?#].*)?$`, 'i');
      const match = normalized.match(assetPattern);
      if (!match) {
        return null;
      }
      const file = match[1];
      const suffix = match[2] ?? '';
      return buildAssetHref(file, suffix);
    };

    this.rewriteLinkHrefs($, 'stylesheet', rewriteAssetHref);
    this.rewriteLinkHrefs($, 'modulepreload', rewriteAssetHref);

    return $.html();
  }

  private rewriteLinkHrefs(
    $: ReturnType<typeof load>,
    rel: 'stylesheet' | 'modulepreload',
    transform: (href: string) => string | null
  ): void {
    $('link[rel][href]').each((_, element) => {
      const $element = $(element);
      if (!this.linkHasRel($element.attr('rel'), rel)) {
        return;
      }

      const currentHref = $element.attr('href');
      const nextHref = transform(currentHref ?? '');
      if (!nextHref || nextHref === currentHref) {
        return;
      }

      $element.attr('href', nextHref);
    });
  }

  private linkHasRel(relAttr: string | undefined, target: string): boolean {
    if (!relAttr) return false;
    const relValue = relAttr
      .split(/\s+/)
      .map(value => value.toLowerCase())
      .filter(Boolean);
    return relValue.includes(target.toLowerCase());
  }

  private resolveSourceImportPath(buildDir: string, sourcePath: string): string {
    const relativePath = path.relative(buildDir, sourcePath).split(path.sep).join('/');
    return relativePath.startsWith('.') ? relativePath : `./${relativePath}`;
  }
}
