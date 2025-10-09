import fs from 'fs';
import path from 'path';
import { execSync } from 'child_process';
import { FileUtils } from '../utils/file.js';
import { TemplateManager } from '../template.js';
import { ConfigManager } from '../config.js';
import type { BuildOptions } from '../../types/index.js';

/**
 * Modern LogicBuilder that leverages Vite for dependency resolution and bundling
 */
export class LogicBuilder {
  private projectPath: string;
  private outputDir: string;
  private fileUtils: FileUtils;
  private templateManager: TemplateManager;
  private configManager: ConfigManager;

  constructor(projectPath: string, outputDir: string) {
    this.projectPath = projectPath;
    this.outputDir = outputDir;
    this.fileUtils = new FileUtils();
    this.templateManager = new TemplateManager();
    this.configManager = new ConfigManager(projectPath);
  }

  async buildLogic(options: BuildOptions = {}): Promise<void> {
    console.log(' Building logic layer...');

    // Get page configurations from ConfigManager
    const pages = this.configManager.getPages();
    const logicFiles = this.discoverLogicFiles(pages);

    if (logicFiles.length === 0) {
      console.log(' No logic files found, skipping logic layer build');
      return;
    }

    // Use Vite to build logic layer with proper dependency resolution
    await this.buildLogicWithVite(logicFiles, pages, options);
    console.log(` Generated logic.js (${logicFiles.length} files combined)`);
  }

  /**
   * Discover logic files based on pages configuration
   */
  private discoverLogicFiles(pages: string[]): string[] {
    const logicFiles: string[] = [];

    // Add lxapp.js or lxapp.ts if it exists
    const lxappJsPath = path.join(this.projectPath, 'lxapp.js');
    const lxappTsPath = path.join(this.projectPath, 'lxapp.ts');

    if (fs.existsSync(lxappTsPath)) {
      logicFiles.push(lxappTsPath);
    } else if (fs.existsSync(lxappJsPath)) {
      logicFiles.push(lxappJsPath);
    }

    // Process each page path
    for (const pagePath of pages) {
      // Remove extension and try .js and .ts
      const basePath = path.join(this.projectPath, pagePath.replace(/\.[^.]+$/, ''));

      // Check which logic file exists
      const jsPath = `${basePath}.js`;
      const tsPath = `${basePath}.ts`;

      if (fs.existsSync(jsPath)) {
        logicFiles.push(jsPath);
      } else if (fs.existsSync(tsPath)) {
        logicFiles.push(tsPath);
      }
    }

    return logicFiles;
  }

  /**
   * Build logic layer using Vite for proper dependency resolution
   */
  private async buildLogicWithVite(logicFiles: string[], pages: string[], options: BuildOptions = {}): Promise<void> {
    const buildDir = path.join(this.projectPath, '.lingxia-build', 'logic');
    
    // Always clean entire directory, including node_modules
    const nodeModulesPath = path.join(buildDir, 'node_modules');
    this.fileUtils.cleanDirectory(buildDir);

    // Create entry file that imports all logic files
    const entryContent = this.createLogicEntry(logicFiles, pages);
    fs.writeFileSync(path.join(buildDir, 'main.js'), entryContent);

    // Prepare package files by copying from project root (preserve and only overwrite)
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

    // Keep root package.json as-is (scripts unchanged) to ensure consistency across layers

    // Install dependencies: prefer npm ci when lock exists
    const hasLock = fs.existsSync(packageLockPath);
    const hasNodeModules = fs.existsSync(nodeModulesPath);
    if (hasNodeModules) {
      // Enforce fresh install per requirement
      fs.rmSync(nodeModulesPath, { recursive: true, force: true });
    }
    {
      console.log(` Installing logic dependencies...`);
      console.log(`  -> ${hasLock ? 'Using npm ci (lock found)' : 'Using npm install (no lock found)'}`);
      if (hasLock) {
        execSync('npm ci', { cwd: buildDir, stdio: 'inherit' });
      } else {
        execSync('npm install', { cwd: buildDir, stdio: 'inherit' });
      }
    }

    // Create Vite config for logic build
    this.createLogicViteConfig(buildDir, options);

    // Copy source files to build directory
    this.copySourceFiles(logicFiles, buildDir);

    // Build with Vite (invoke directly to avoid using root scripts in copied package.json)
    execSync('npx vite build', { cwd: buildDir, stdio: 'inherit' });

    // Copy built logic.js to output
    const builtLogicPath = path.join(buildDir, 'dist', 'main.iife.js');
    const outputPath = path.join(this.outputDir, 'logic.js');
    fs.copyFileSync(builtLogicPath, outputPath);
  }

  /**
   * Create entry file that imports all logic files
   */
  private createLogicEntry(logicFiles: string[], pages: string[]): string {
    const imports: string[] = [];

    for (let i = 0; i < logicFiles.length; i++) {
      const logicFile = logicFiles[i];
      const fileName = path.basename(logicFile);

      // Process the logic file to add path parameter to Page calls
      if (fileName !== 'lxapp.js' && fileName !== 'lxapp.ts') {
        const pagePath = this.getPagePathFromConfig(logicFile, pages);
        const baseName = path.basename(logicFile, path.extname(logicFile));
        const logicDir = path.dirname(path.relative(this.projectPath, logicFile));
        const processedFileName = `${logicDir.replace(/[\/\\]/g, '_')}_${baseName}_processed${path.extname(logicFile)}`;

        // Read, process, and write the modified file
        this.processLogicFileForPath(logicFile, processedFileName, pagePath);
        imports.push(`import './${processedFileName}';`);
      } else {
        // For lxapp files, import as-is
        const relativePath = `./${fileName}`;
        imports.push(`import '${relativePath}';`);
      }
    }

    return imports.join('\n');
  }

  /**
   * Process logic file to add path parameter to Page calls
   */
  private processLogicFileForPath(sourceFile: string, targetFileName: string, pagePath: string): void {
    const buildDir = path.join(this.projectPath, '.lingxia-build', 'logic');
    const targetPath = path.join(buildDir, targetFileName);

    // Read the source file
    let content = fs.readFileSync(sourceFile, 'utf-8');

    // Find and modify Page({ ... }) calls to add path as the last parameter
    // Use a simpler approach: find Page({ and then find the matching closing })
    const lines = content.split('\n');
    const result = [];
    let inPageCall = false;
    let braceCount = 0;

    for (let i = 0; i < lines.length; i++) {
      const line = lines[i];

      // Check if this line starts a Page call
      if (!inPageCall && /^\s*Page\s*\(\s*\{/.test(line)) {
        inPageCall = true;
        braceCount = 1;
        result.push(line);
        continue;
      }

      if (inPageCall) {
        // Count braces to find the end of the Page call
        for (const char of line) {
          if (char === '{') braceCount++;
          if (char === '}') braceCount--;
        }

        // If we've closed all braces, this is the end of the Page call
        if (braceCount === 0) {
          // Replace the closing }); with }, 'path');
          const modifiedLine = line.replace(/\}\s*\)\s*;?\s*$/, `}, '${pagePath}');`);
          result.push(modifiedLine);
          inPageCall = false;
        } else {
          result.push(line);
        }
      } else {
        result.push(line);
      }
    }

    content = result.join('\n');

    // Write the processed file
    fs.writeFileSync(targetPath, content);
  }

  /**
   * Get page path from pages configuration
   */
  private getPagePathFromConfig(logicFilePath: string, pages: string[]): string {
    // Extract the directory and filename from the logic file path
    const relativePath = path.relative(this.projectPath, logicFilePath);
    const logicDir = path.dirname(relativePath);
    const logicBaseName = path.basename(logicFilePath, path.extname(logicFilePath));

    // Find the page path that corresponds to this logic file
    for (const pagePath of pages) {
      const pageDir = path.dirname(pagePath);
      const pageBaseName = path.basename(pagePath, path.extname(pagePath));

      // Check if this logic file corresponds to this page
      // Both directory and base name should match
      if (pageDir === logicDir && pageBaseName === logicBaseName) {
        return pagePath;
      }
    }

    // Fallback - this shouldn't happen if lxapp.json is correct
    return `${logicBaseName}.html`;
  }



  /**
   * Create Vite config for logic build using TemplateManager
   */
  private createLogicViteConfig(buildDir: string, options: BuildOptions = {}): void {
    const isProd = options.prod || false;
    const viteConfig = this.templateManager.getViteConfig('logic', this.projectPath, isProd);
    fs.writeFileSync(path.join(buildDir, 'vite.config.js'), viteConfig);
  }

  /**
   * Copy source files to build directory
   */
  private copySourceFiles(logicFiles: string[], buildDir: string): void {
    for (const logicFile of logicFiles) {
      const fileName = path.basename(logicFile);
      const destPath = path.join(buildDir, fileName);
      fs.copyFileSync(logicFile, destPath);
    }
  }
}
