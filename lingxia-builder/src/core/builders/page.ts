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

    // Install dependencies
    console.log(` Installing ${processor.getFrameworkName()} dependencies...`);
    this.dependencyResolver.createPackageJson(framework, buildDir);

    // Add build scripts to package.json
    const packageJsonPath = path.join(buildDir, 'package.json');
    const packageJson = this.fileUtils.readJsonFile(packageJsonPath);
    if (packageJson) {
      packageJson.scripts = {
        build: 'vite build',
        dev: 'vite'
      };
      this.fileUtils.writeJsonFile(packageJsonPath, packageJson);
    }

    // Copy src directory and resolve dependencies
    await this.copySrcDirectory(buildDir);
    await this.copyTailwindConfig(buildDir);
    await this.copyTailwindCss(buildDir);

    // Setup framework-specific build
    await processor.setupBuild(buildDir, page, pageFiles, pageFunctions);

    // Create Vite config using framework processor with build options
    await processor.createViteConfig(buildDir, options);

    // Install and build
    execSync('npm install', { cwd: buildDir, stdio: 'inherit' });
    console.log(`${processor.getFrameworkName()} dependencies installed`);

    console.log(` Building ${processor.getFrameworkName()} page with Vite...`);
    execSync('npm run build', { cwd: buildDir, stdio: 'inherit' });

    // Generate function bridge script (unified for all frameworks)
    const bridgeScript = this.templateManager.generateFunctionBridge(pageFunctions);

    // Generate output using framework processor
    const buildResult = { distDir: path.join(buildDir, 'dist') };
    await processor.generateOutput(page, pageFiles, buildResult, bridgeScript);
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

      // Update content paths to scan build directory files
      configContent = configContent.replace(
        /content:\s*\[[\s\S]*?\]/,
        `content: [
    './App.tsx',
    './App.vue',
    './main.jsx',
    './main.js',
    './src/**/*.{ts,tsx,js,jsx,vue}',
    './**/*.{html,js,ts,jsx,tsx,vue}',
  ]`
      );

      const destConfigPath = path.join(buildDir, 'tailwind.config.js');
      fs.writeFileSync(destConfigPath, configContent);
      console.log(` Updated Tailwind config for build directory`);
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
