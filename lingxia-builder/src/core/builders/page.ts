import * as fs from 'fs';
import * as path from 'path';
import { execSync } from 'child_process';
import type { Page, PageFiles, BuildOptions } from '../../types/index.js';
import { FileUtils } from '../utils/file.js';
import { DependencyResolver } from './deps.js';

import { TemplateManager } from '../template.js';
import { FrameworkFactory } from '../frameworks/factory.js';

export class PageProcessor {
  private projectPath: string;
  private outputDir: string;
  private fileUtils: FileUtils;
  private dependencyResolver: DependencyResolver;
  private templateManager: TemplateManager;

  constructor(projectPath: string, outputDir: string) {
    this.projectPath = projectPath;
    this.outputDir = outputDir;
    this.fileUtils = new FileUtils();
    this.dependencyResolver = new DependencyResolver(projectPath);
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

    // Prepare package files by copying from project root, then override build script for Vite
    console.log(` Installing ${processor.getFrameworkName()} dependencies...`);
    const rootPackageJson = path.join(this.projectPath, 'package.json');
    const rootPackageLock = path.join(this.projectPath, 'package-lock.json');
    const packageJsonPath = path.join(buildDir, 'package.json');
    const packageLockPath = path.join(buildDir, 'package-lock.json');

    if (fs.existsSync(rootPackageJson)) {
      fs.copyFileSync(rootPackageJson, packageJsonPath);
    }
    if (fs.existsSync(rootPackageLock)) {
      fs.copyFileSync(rootPackageLock, packageLockPath);
    }

    // Copy src directory and resolve dependencies
    await this.copySrcDirectory(buildDir);
    await this.copyTailwindConfig(buildDir);
    await this.copyTailwindCss(buildDir);

    // Setup framework-specific build
    await processor.setupBuild(buildDir, page, pageFiles, pageFunctions);

    // Create Vite config using framework processor with build options
    await processor.createViteConfig(buildDir, options);

    // Install and build (prefer npm ci)
    const hasLockVite = fs.existsSync(packageLockPath);
    if (hasLockVite) {
      console.log('  -> Using npm ci (lock found)');
      execSync('npm ci', { cwd: buildDir, stdio: 'inherit' });
    } else {
      console.log('  -> Using npm install (no lock found)');
      execSync('npm install', { cwd: buildDir, stdio: 'inherit' });
    }
    console.log(`${processor.getFrameworkName()} dependencies installed`);

    console.log(` Building ${processor.getFrameworkName()} page with Vite...`);
    execSync('npx vite build', { cwd: buildDir, stdio: 'inherit' });

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
    console.error('buildPagesBatch', framework, items, options);
    const processor = FrameworkFactory.createProcessor(
      framework,
      this.projectPath,
      this.outputDir,
      this.templateManager.getTemplatesDir()
    );

    const buildDir = path.join(this.projectPath, '.lingxia-build', `view-${framework}`);
    this.fileUtils.cleanDirectory(buildDir);

    // Shared package.json: copy from root and override build script for Vite
    console.log(` Installing ${processor.getFrameworkName()} dependencies...`);
    const rootPackageJson = path.join(this.projectPath, 'package.json');
    const rootPackageLock = path.join(this.projectPath, 'package-lock.json');
    const packageJsonPath = path.join(buildDir, 'package.json');
    const packageLockPath = path.join(buildDir, 'package-lock.json');
    if (fs.existsSync(rootPackageJson)) {
      fs.copyFileSync(rootPackageJson, packageJsonPath);
    }
    if (fs.existsSync(rootPackageLock)) {
      fs.copyFileSync(rootPackageLock, packageLockPath);
    }

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

    // Write multi-entry vite.config.js locally (include framework plugins, disable shared chunks)
    const viteConfig = (() => {
      if (framework === 'react') {
        return `import react from '@vitejs/plugin-react';\nimport tailwindcss from 'tailwindcss';\nimport autoprefixer from 'autoprefixer';\nimport { defineConfig } from 'vite'\nexport default defineConfig({\n  plugins: [react()],\n  css: { postcss: { plugins: [tailwindcss, autoprefixer] } },\n  build: {\n    outDir: 'dist',\n    emptyOutDir: true,\n    rollupOptions: {\n      input: ${JSON.stringify(inputs, null, 2)},\n      output: {\n        entryFileNames: 'pages/[name]/[name].js',\n        chunkFileNames: 'assets/[name]-[hash].js',\n        assetFileNames: 'assets/[name].[ext]',
        manualChunks: null
      }
    },\n    cssCodeSplit: false\n  }\n})\n`;
      }
      // vue
      return `import vue from '@vitejs/plugin-vue';\nimport tailwindcss from 'tailwindcss';\nimport autoprefixer from 'autoprefixer';\nimport { defineConfig } from 'vite'\nexport default defineConfig({\n  plugins: [vue()],\n  css: { postcss: { plugins: [tailwindcss, autoprefixer] } },\n  build: {\n    outDir: 'dist',\n    emptyOutDir: true,\n    rollupOptions: {\n      input: ${JSON.stringify(inputs, null, 2)},\n      output: {\n        entryFileNames: 'pages/[name]/[name].js',\n        chunkFileNames: 'assets/[name]-[hash].js',\n        assetFileNames: 'assets/[name].[ext]',
        manualChunks: null
      }
    },\n    cssCodeSplit: false\n  }\n})\n`;
    })();
    fs.writeFileSync(path.join(buildDir, 'vite.config.js'), viteConfig);

    // Install deps and build once (prefer npm ci when lock exists)
    const hasLock = fs.existsSync(path.join(buildDir, 'package-lock.json'));
    if (hasLock) {
      console.log('  -> Using npm ci (lock found)');
      execSync('npm ci', { cwd: buildDir, stdio: 'inherit' });
    } else {
      console.log('  -> Using npm install (no lock found)');
      execSync('npm install', { cwd: buildDir, stdio: 'inherit' });
    }
    console.log(`${processor.getFrameworkName()} dependencies installed`);
    console.log(` Building ${processor.getFrameworkName()} pages with Vite (multi-entry)...`);
    execSync('npx vite build', { cwd: buildDir, stdio: 'inherit' });

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
