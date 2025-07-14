import * as fs from 'fs';
import * as path from 'path';
import { FrameworkProcessor } from './base.js';
import type { Page, PageFiles } from '../../types/index.js';
import { getPageTitle } from '../utils/page.js';
import { FileUtils } from '../utils/file.js';
import { TemplateManager } from '../template.js';

/**
 * Vue framework processor
 * Uses templates and framework-specific logic
 */
export class VueProcessor extends FrameworkProcessor {
  private fileUtils: FileUtils;
  private templateManager: TemplateManager;

  constructor(projectPath: string, outputDir: string, templatesDir: string) {
    super(projectPath, outputDir, templatesDir);
    this.fileUtils = new FileUtils();
    this.templateManager = new TemplateManager();
  }

  getFrameworkName(): string {
    return 'Vue';
  }

  getExtensions(): string[] {
    return ['.vue'];
  }

  getDependencies(): { dependencies: any; devDependencies: any } {
    return this.templateManager.getFrameworkDependencies('vue');
  }

  async createViteConfig(buildDir: string, options: any = {}): Promise<any> {
    // Generate dynamic Vite config based on build mode
    const isProd = options.prod || false;
    const viteConfig = this.templateManager.getViteConfig('vue', this.projectPath, isProd);
    const destPath = path.join(buildDir, 'vite.config.js');
    fs.writeFileSync(destPath, viteConfig);
    return null; // Config is written as file, not returned as object
  }

  async setupBuild(
    buildDir: string,
    page: Page,
    pageFiles: PageFiles,
    pageFunctions: string[]
  ): Promise<void> {
    // Copy page component
    let content = fs.readFileSync(pageFiles.view.path, 'utf-8');
    content = content.replace(/\.\.\/\.\.\/src\//g, './src/');
    fs.writeFileSync(path.join(buildDir, 'App.vue'), content);

    // Copy framework templates
    this.copyTemplates(buildDir);

    // Process templates with page-specific data
    await this.processTemplates(buildDir, page, pageFiles, pageFunctions);
  }

  async generateOutput(
    page: Page,
    pageFiles: PageFiles,
    buildResult: { distDir: string },
    bridgeScript: string
  ): Promise<void> {
    const pageOutputDir = path.join(this.outputDir, path.dirname(page.path));
    const baseName = path.basename(page.path, path.extname(page.path));

    // Copy built assets
    const builtIndexHtml = path.join(buildResult.distDir, 'index.html');
    const builtMainJs = path.join(buildResult.distDir, 'main.js');
    const builtCss = path.join(buildResult.distDir, 'assets');

    let htmlContent = fs.readFileSync(builtIndexHtml, 'utf-8');

    // Ensure output directory exists
    this.fileUtils.ensureDirectory(pageOutputDir);

    // Process CSS - merge original page CSS with Vite-generated CSS
    let finalCssContent = '';

    // First, add original page CSS if it exists
    if (pageFiles.style.exists) {
      const originalCss = fs.readFileSync(pageFiles.style.path, 'utf-8');
      finalCssContent += originalCss + '\n\n';
    }

    // Then, add Vite-generated CSS
    if (fs.existsSync(builtCss)) {
      const cssFiles = fs.readdirSync(builtCss).filter(f => f.endsWith('.css'));
      if (cssFiles.length > 0) {
        const viteCss = fs.readFileSync(path.join(builtCss, cssFiles[0]), 'utf-8');
        finalCssContent += viteCss;
      }
    }

    // Write merged CSS
    if (finalCssContent.trim()) {
      fs.writeFileSync(path.join(pageOutputDir, `${baseName}.css`), finalCssContent);
      console.log(` Generated CSS file: ${baseName}.css`);
    }

    // Process JS
    if (fs.existsSync(builtMainJs)) {
      fs.copyFileSync(builtMainJs, path.join(pageOutputDir, 'view.js'));
      console.log(` Generated JS file: view.js`);
    }

    // Copy page config
    if (pageFiles.config.exists) {
      const configOutputPath = path.join(pageOutputDir, `${baseName}.json`);
      fs.copyFileSync(pageFiles.config.path, configOutputPath);
      console.log(` Generated page config: ${baseName}.json`);
    }

    // Fix HTML paths and inject bridge script
    htmlContent = this.fixHtmlPaths(htmlContent, baseName);
    htmlContent = htmlContent.replace(
      '</body>',
      `<script>\n${bridgeScript}\n</script>\n</body>`
    );

    // Write final component file
    const componentOutputPath = path.join(pageOutputDir, `${baseName}.vue`);
    fs.writeFileSync(componentOutputPath, htmlContent);
    console.log(` Generated single page file: ${baseName}.vue`);
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

    // Process main.js
    const mainJsPath = path.join(buildDir, 'main.js');
    if (fs.existsSync(mainJsPath)) {
      let mainJs = fs.readFileSync(mainJsPath, 'utf-8');

      // Inject page functions
      const bridgeScript = this.templateManager.generateFunctionBridge(pageFunctions);
      mainJs = mainJs.replace('/* {{PAGE_FUNCTIONS}} */', bridgeScript);

      fs.writeFileSync(mainJsPath, mainJs);
    }

    console.log(` Setup Vue templates with page functions`);
  }

  /**
   * Fix HTML paths to use relative paths for Vue
   */
  private fixHtmlPaths(htmlContent: string, baseName: string): string {
    let fixedContent = htmlContent;

    // JS path: /main.js -> ./view.js
    fixedContent = fixedContent.replace(
      /<script[^>]*src="\/main\.js"[^>]*><\/script>/g,
      '<script type="module" src="./view.js"></script>'
    );

    // CSS path: /assets/*.css -> ./baseName.css
    fixedContent = fixedContent.replace(
      /<link[^>]*href="\/assets\/[^"]*\.css"[^>]*>/g,
      `<link rel="stylesheet" href="./${baseName}.css">`
    );

    return fixedContent;
  }
}
