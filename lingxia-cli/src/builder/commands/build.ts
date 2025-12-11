import fs from 'fs';
import path from 'path';
import { spawn } from 'child_process';
import type { BuildOptions, Page } from '../types/index.js';
import { ViewBuilder } from '../core/builders/view.js';
import { LogicBuilder } from '../core/builders/logic.js';
import { FileUtils } from '../core/utils/file.js';
import { detectPageType } from '../core/utils/page.js';
import { ConfigManager } from '../core/config.js';

const fileUtils = new FileUtils();

export async function buildCommand(options: BuildOptions): Promise<void> {
  const projectPath = process.cwd();
  const outputDir = path.join(projectPath, 'dist');
  const configManager = new ConfigManager(projectPath);

  // Map CLI options to BuildOptions
  const buildOptions: BuildOptions = {
    dev: options.dev,
    prod: options.prod
  };

  console.log('🚀 Starting LingXia build...');
  console.log(` Project: ${projectPath}`);
  console.log(` Output: ${outputDir}`);
  console.log(` View bundler: Vite`);

  try {
    // Clean and prepare output directory
    fileUtils.cleanDirectory(outputDir);

    // Discover pages
    const pages = discoverPages(projectPath, configManager);
    console.log(` Found ${pages.length} pages: ${pages.map(p => p.name).join(', ')}`);

    if (pages.length === 0) {
      console.warn('⚠️ No pages found in the project');
      return;
    }

    const startTime = Date.now();

    const only = process.env.LINGXIA_ONLY?.toLowerCase();
    
    if (only === 'logic') {
      console.log('▶ Building logic layer only...');
      const logicBuilder = new LogicBuilder(projectPath, outputDir);
      await logicBuilder.buildLogic(buildOptions);
    } else if (only === 'view') {
      console.log('▶ Building view layer only...');
      const viewBuilder = new ViewBuilder(projectPath, outputDir);
      await viewBuilder.buildPages(pages, buildOptions);
    } else {
      console.log('▶ Building logic and view layers in parallel...');
      const logicBuilder = new LogicBuilder(projectPath, outputDir);
      const viewBuilder = new ViewBuilder(projectPath, outputDir);

      await Promise.all([
        logicBuilder.buildLogic(buildOptions).then(() => console.log('  ✔ Logic layer built')),
        viewBuilder.buildPages(pages, buildOptions).then(() => console.log('  ✔ View layer built'))
      ]);
    }

    const endTime = Date.now();
    const buildTime = ((endTime - startTime) / 1000).toFixed(2);

    cleanupLingxiaBuild(projectPath);
    const packageInfo = readPackageInfo(projectPath);
    const packagePath = await packageDist(outputDir, projectPath, packageInfo);
    const relativePackagePath =
      path.relative(projectPath, packagePath) || packagePath;

    console.log('✅ Build completed successfully!');
    console.log(` ⏱ Completed in ${buildTime}s`);
    console.log(` 📁 Output directory: ${outputDir}`);
    console.log(` 📦 Package: ${relativePackagePath}`);

  } catch (error) {
    console.error('❌ Build failed:', error instanceof Error ? error.message : String(error));
    process.exit(1);
  }
}

function discoverPages(projectPath: string, configManager: ConfigManager): Page[] {
  const pagesPaths = configManager.getPages();

  const pages: Page[] = [];

  for (const pagePath of pagesPaths) {
    // Check if page file exists
    const fullPath = path.join(projectPath, pagePath);
    if (!fs.existsSync(fullPath)) {
      console.warn(`⚠️ Page file not found: ${pagePath}`);
      continue;
    }

    // Extract page info
    const pageType = detectPageType(pagePath);
    const pageDir = path.dirname(pagePath);
    const baseName = path.basename(pagePath, path.extname(pagePath));

    // Create page name from directory structure
    let pageName = pageDir;
    if (pageDir.startsWith('pages/')) {
      pageName = pageDir.substring(6); // Remove 'pages/' prefix
    }
    if (!pageName) {
      pageName = baseName;
    }

    pages.push({
      path: pagePath, // Full path from lxapp.json
      name: pageName,
      type: pageType
    });
  }

  return pages;
}

function cleanupLingxiaBuild(projectPath: string): void {
  const tempDir = path.join(projectPath, '.lingxia-build');
  if (fs.existsSync(tempDir)) {
    fs.rmSync(tempDir, { recursive: true, force: true });
  }
}

type PackageInfo = {
  name?: string;
  version?: string;
};

function readPackageInfo(projectPath: string): PackageInfo {
  const pkgPath = path.join(projectPath, 'package.json');
  if (!fs.existsSync(pkgPath)) {
    return {};
  }
  try {
    const raw = fs.readFileSync(pkgPath, 'utf-8');
    return JSON.parse(raw) as PackageInfo;
  } catch (error) {
    console.warn(
      '⚠️ Failed to read package.json:',
      error instanceof Error ? error.message : String(error)
    );
    return {};
  }
}

async function packageDist(
  distDir: string,
  projectPath: string,
  pkgInfo: PackageInfo
): Promise<string> {
  if (!fs.existsSync(distDir)) {
    throw new Error('Dist directory not found, cannot package build output.');
  }

  const baseName = sanitizeName(pkgInfo.name ?? 'lingxia-app');
  const version = sanitizeVersion(pkgInfo.version ?? '0.0.0');
  const archiveName = `${baseName}-${version}.tar.zstd`;
  const archivePath = path.join(projectPath, archiveName);

  if (fs.existsSync(archivePath)) {
    fs.rmSync(archivePath, { force: true });
  }

  const distRelative = path.relative(projectPath, distDir) || distDir;
  await runTar(['--zstd', '-cf', archivePath, distRelative], projectPath);
  return archivePath;
}

function sanitizeName(name: string): string {
  const fallback = 'lingxia-app';
  if (!name || typeof name !== 'string') {
    return fallback;
  }
  const cleaned = name.trim().replace(/[^a-zA-Z0-9._-]/g, '_');
  return cleaned.length > 0 ? cleaned : fallback;
}

function sanitizeVersion(version: string): string {
  const fallback = '0.0.0';
  if (!version || typeof version !== 'string') {
    return fallback;
  }
  const cleaned = version.trim().replace(/[^0-9a-zA-Z._-]/g, '_');
  return cleaned.length > 0 ? cleaned : fallback;
}

function runTar(args: string[], cwd: string): Promise<void> {
  return new Promise((resolve, reject) => {
    const child = spawn('tar', args, { cwd, stdio: 'inherit' });
    child.on('error', err => reject(err));
    child.on('exit', code => {
      if (code === 0) {
        resolve();
      } else {
        reject(new Error(`tar exited with code ${code ?? 'unknown'}`));
      }
    });
  });
}
