import * as fs from 'fs';
import * as path from 'path';
import type { BuildOptions, Page, PageFiles } from '../../types/index.js';
import { FileUtils } from '../utils/file.js';
import { TemplateManager } from '../template.js';
import { FrameworkFactory } from '../frameworks/factory.js';

export class PageProcessor {
  private projectPath: string;
  private outputDir: string;
  private fileUtils: FileUtils;
  private templateManager: TemplateManager;

  constructor(projectPath: string, outputDir: string) {
    this.projectPath = projectPath;
    this.outputDir = outputDir;
    this.fileUtils = new FileUtils();
    this.templateManager = new TemplateManager();
  }

  /**
   * Build page using framework-specific processor
   */
  async buildPage(page: Page, pageFiles: PageFiles, pageFunctions: string[], options: BuildOptions = {}): Promise<void> {
    // Detect framework from page file
    const framework = FrameworkFactory.detectFramework(pageFiles.view.path);

    // Create framework processor
    const processor = FrameworkFactory.createProcessor(
      framework,
      this.projectPath,
      this.outputDir,
      this.templateManager.getTemplatesDir()
    );

    console.log(`Building page: ${page.path}`);
    console.log(`Extracted ${pageFunctions.length} functions: ${pageFunctions.join(', ')}`);

    if (framework === 'html') {
      // HTML pages don't need Vite build - generate bridge script here
      const bridgeScript = this.templateManager.generateFunctionBridge(pageFunctions);
      await processor.generateOutput(page, pageFiles, { distDir: '' }, bridgeScript);
    } else {
      // Framework pages need Vite build
      await this.buildFrameworkPage(processor, page, pageFiles, pageFunctions, options);
    }

    console.log(`Page built: ${page.path}`);
  }

  /**
   * Build framework page using Vite
   */
  private async buildFrameworkPage(
    processor: any,
    page: Page,
    pageFiles: PageFiles,
    pageFunctions: string[],
    options: BuildOptions = {}
  ): Promise<void> {
    const framework = processor.getFrameworkName().toLowerCase();
    const buildDir = path.join(this.projectPath, '.lingxia-build', path.dirname(page.path));

    // Clean and setup build directory
    this.fileUtils.cleanDirectory(buildDir);

    // Copy src directory and resolve dependencies
    await this.copySrcDirectory(buildDir);
    await this.copyTailwindConfig(buildDir);
    await this.copyTailwindCss(buildDir);

    // Setup framework-specific build
    await processor.setupBuild(buildDir, page, pageFiles, pageFunctions);

    console.log(` Building ${processor.getFrameworkName()} page with bundled Vite...`);
    await this.runViteBuild(buildDir, framework as 'react' | 'vue', {
      options,
      inputs: { index: path.join(buildDir, 'index.html') },
      output: {
        entryFileNames: 'main.js',
        chunkFileNames: 'chunks/[name]-[hash].js',
        assetFileNames: 'assets/[name].[ext]'
      },
      cssCodeSplit: false,
      esbuild: framework === 'react' ? { jsx: 'automatic' } : undefined,
      target: 'es2015'
    });

    // Generate function bridge script (unified for all frameworks)
    const bridgeScript = this.templateManager.generateFunctionBridge(pageFunctions);

    // Generate output using framework processor
    const buildResult = { distDir: path.join(buildDir, 'dist') };
    await processor.generateOutput(page, pageFiles, buildResult, bridgeScript);
  }

  /**
   * Batch build multiple pages for a single framework using Vite multi-entry.
   * Writes a dedicated multi-entry vite.config.js (no framework API changes),
   * installs once, builds once, then normalizes per-entry to existing processor
   * expectations by temporarily mapping <entry>.html/js to index.html/main.js.
   */
  async buildPagesBatch(
    framework: 'react' | 'vue',
    items: { page: Page; pageFiles: PageFiles; pageFunctions: string[] }[],
    options: BuildOptions = {}
  ): Promise<void> {
    if (items.length === 0) return;
    const processor = FrameworkFactory.createProcessor(
      framework,
      this.projectPath,
      this.outputDir,
      this.templateManager.getTemplatesDir()
    );

    const buildDir = path.join(this.projectPath, '.lingxia-build', `view-${framework}`);
    this.fileUtils.cleanDirectory(buildDir);

    // Shared package.json: copy from root and override build script for Vite
    // Copy shared assets/config
    await this.copySrcDirectory(buildDir);
    await this.copyTailwindConfig(buildDir);
    await this.copyTailwindCss(buildDir);

    // Prepare per-page subdirs and collect multi-entry inputs
    const inputs: Record<string, string> = {};
    const entryNameByPagePath: Record<string, string> = {};
    for (const { page, pageFiles, pageFunctions } of items) {
      const entryName = (page as any).name || path.dirname(page.path).replace(/^pages\//, '') || path.basename(page.path, path.extname(page.path));
      const subDir = path.join(buildDir, 'pages', entryName);
      this.fileUtils.ensureDirectory(subDir);
      // Ensure tailwind.css exists in subDir for relative imports (./tailwind.css)
      const rootTailwind = path.join(buildDir, 'tailwind.css');
      const subTailwind = path.join(subDir, 'tailwind.css');
      if (fs.existsSync(rootTailwind) && !fs.existsSync(subTailwind)) {
        fs.copyFileSync(rootTailwind, subTailwind);
      }

      await processor.setupBuild(subDir, page, pageFiles, pageFunctions);
      inputs[entryName] = path.join(subDir, 'index.html');
      entryNameByPagePath[page.path] = entryName;
    }

    console.log(` Building ${processor.getFrameworkName()} pages with bundled Vite (multi-entry)...`);
    await this.runViteBuild(buildDir, framework, {
      options,
      inputs,
      output: {
        entryFileNames: 'pages/[name]/[name].js',
        chunkFileNames: 'assets/[name]-[hash].js',
        assetFileNames: 'assets/[name].[ext]',
        manualChunks: null
      },
      cssCodeSplit: false
    });

    const distDir = path.join(buildDir, 'dist');

    // Copy assets from build dist to final output
    const buildAssetsDir = path.join(distDir, 'assets');
    if (fs.existsSync(buildAssetsDir)) {
      const finalAssetsDir = path.join(this.outputDir, 'assets');
      this.fileUtils.ensureDirectory(finalAssetsDir);
      await this.fileUtils.copyDirectory(buildAssetsDir, finalAssetsDir);
      console.log(` Copied assets to final output directory`);
    }

    // For each entry, normalize files to index.html/main.js temporarily
    for (const { page, pageFiles, pageFunctions } of items) {
      const entryName = entryNameByPagePath[page.path];
      const entryHtml = path.join(distDir, 'pages', entryName, 'index.html');
      const entryJs = path.join(distDir, 'pages', entryName, `${entryName}.js`);

      // Backup existing generic files if present
      const genericHtml = path.join(distDir, 'index.html');
      const genericJs = path.join(distDir, 'main.js');
      const backups: { html?: string; js?: string } = {};
      if (fs.existsSync(genericHtml)) {
        backups.html = path.join(distDir, `__index_backup_${Date.now()}.html`);
        fs.renameSync(genericHtml, backups.html);
      }
      if (fs.existsSync(genericJs)) {
        backups.js = path.join(distDir, `__main_backup_${Date.now()}.js`);
        fs.renameSync(genericJs, backups.js);
      }

      // Map entry files to generic names expected by processors
      fs.copyFileSync(entryHtml, genericHtml);
      if (fs.existsSync(entryJs)) {
        fs.copyFileSync(entryJs, genericJs);
      }

      // Generate bridge and output via existing processor API
      const bridgeScript = this.templateManager.generateFunctionBridge(pageFunctions);
      await processor.generateOutput(page, pageFiles, { distDir }, bridgeScript);

      // Cleanup generic files
      if (fs.existsSync(genericHtml)) fs.unlinkSync(genericHtml);
      if (fs.existsSync(genericJs)) fs.unlinkSync(genericJs);
      // Restore backups if any (not strictly needed since next loop overwrites again)
      if (backups.html && fs.existsSync(backups.html)) fs.unlinkSync(backups.html);
      if (backups.js && fs.existsSync(backups.js)) fs.unlinkSync(backups.js);
    }
  }

  private async runViteBuild(
    buildDir: string,
    framework: 'react' | 'vue',
    config: {
      options: BuildOptions;
      inputs: Record<string, string>;
      output: Record<string, unknown>;
      cssCodeSplit?: boolean;
      esbuild?: Record<string, unknown>;
      target?: string;
    }
  ): Promise<void> {
    const { build } = await import('vite');
    const plugins = await this.resolveFrameworkPlugins(framework);
    const css = await this.createCssConfig(buildDir);
    const isDev = Boolean(config.options.dev);
    const isProd = Boolean(config.options.prod);

    await build({
      configFile: false,
      root: buildDir,
      logLevel: 'warn',
      mode: isDev ? 'development' : isProd ? 'production' : undefined,
      plugins,
      css,
      esbuild: config.esbuild,
      build: {
        outDir: path.join(buildDir, 'dist'),
        emptyOutDir: true,
        rollupOptions: {
          input: config.inputs,
          output: config.output
        },
        cssCodeSplit: config.cssCodeSplit ?? true,
        target: config.target,
        minify: isProd ? 'esbuild' : false,
        sourcemap: isDev
      }
    });
  }

  private async resolveFrameworkPlugins(framework: 'react' | 'vue') {
    if (framework === 'react') {
      const reactModule = await import('@vitejs/plugin-react');
      const pluginFactory = (reactModule as any).default ?? reactModule;
      return [pluginFactory()];
    }
    const vueModule = await import('@vitejs/plugin-vue');
    const pluginFactory = (vueModule as any).default ?? vueModule;
    return [pluginFactory()];
  }

  private async createCssConfig(buildDir: string) {
    const tailwindConfigPath = path.join(buildDir, 'tailwind.config.js');
    if (!fs.existsSync(tailwindConfigPath)) {
      return undefined;
    }

    const tailwindModule = await import('tailwindcss');
    const autoprefixerModule = await import('autoprefixer');
    const tailwindcss = (tailwindModule as any).default ?? tailwindModule;
    const autoprefixer = (autoprefixerModule as any).default ?? autoprefixerModule;

    return {
      postcss: {
        plugins: [tailwindcss, autoprefixer]
      }
    };
  }

  private async copySrcDirectory(buildDir: string): Promise<void> {
    const srcDir = path.join(this.projectPath, 'src');
    const srcDestDir = path.join(buildDir, 'src');

    if (fs.existsSync(srcDir)) {
      await this.fileUtils.copyDirectory(srcDir, srcDestDir);
      console.log(` Copied src directory to build directory`);
    }
  }

  private async copyTailwindConfig(buildDir: string): Promise<void> {
    const tailwindConfigPath = path.join(this.projectPath, 'tailwind.config.js');
    if (fs.existsSync(tailwindConfigPath)) {
      let configContent = fs.readFileSync(tailwindConfigPath, 'utf-8');
      if (!/content\s*:\s*\[/.test(configContent)) {
        configContent = configContent.replace(
          /(module\.exports\s*=\s*\{[\s\S]*?)(theme\s*:|plugins\s*:|}\s*;?)/,
          (_m, head: string, tail: string) =>
            `${head}content: [\n    './index.html',\n    './App.{tsx,vue}',\n    './main.{js,jsx}',\n    './src/**/*.{js,jsx,ts,tsx,vue}',\n    './pages/**/*.{ts,tsx}'\n  ],\n  ${tail}`
        );
      } else {
        const hasNodeModulesExclusion = /!\.?\/?node_modules\//.test(configContent);
        const hasTempBuildExclusion = /!\.?\/?\.lingxia-build\//.test(configContent);
        if (!hasNodeModulesExclusion || !hasTempBuildExclusion) {
          configContent = configContent.replace(
            /(content\s*:\s*\[)([\s\S]*?)(\])/,
            (_m, start: string, inner: string, end: string) => {
              const additions: string[] = [];
              if (!hasNodeModulesExclusion) additions.push("'!./node_modules/**'");
              if (!hasTempBuildExclusion) additions.push("'!./.lingxia-build/**'");
              const sep = inner.trim().length > 0 && !inner.trim().endsWith(',') ? ',\n' : '';
              return `${start}${inner}${sep}  ${additions.join(', ')}\n${end}`;
            }
          );
        }
      }

      const destConfigPath = path.join(buildDir, 'tailwind.config.js');
      fs.writeFileSync(destConfigPath, configContent);
      console.log(` Copied Tailwind config (preserved content patterns)`);
    } else {
      console.log(`ℹ️ No Tailwind config found, skipping Tailwind setup`);
    }
  }

  private async copyTailwindCss(buildDir: string): Promise<void> {
    const tailwindCssPath = path.join(this.projectPath, 'tailwind.css');
    const destCssPath = path.join(buildDir, 'tailwind.css');

    if (fs.existsSync(tailwindCssPath)) {
      fs.copyFileSync(tailwindCssPath, destCssPath);
      console.log(` Copied tailwind.css to build directory`);
    }
  }
}
