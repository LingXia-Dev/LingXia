import fs from 'fs';
import path from 'path';
import { spawn } from 'child_process';
import type { BuildOptions, Page } from '../types/index.js';
import { ViewBuilder } from '../core/builders/view.js';
import { LogicBuilder } from '../core/builders/logic.js';
import { FileUtils } from '../core/utils/file.js';
import { detectPageType } from '../core/utils/page.js';
import { ConfigManager } from '../core/config.js';
import { loadLxappBuildConfig } from '../core/config/build-config.js';

const fileUtils = new FileUtils();

export async function buildCommand(options: BuildOptions): Promise<void> {
  const projectPath = process.cwd();
  const configManager = new ConfigManager(projectPath);

  const hasLxappConfig = fs.existsSync(path.join(projectPath, 'lxapp.json'));
  const hasPluginConfig = fs.existsSync(path.join(projectPath, 'lxplugin.json'));
  const isPluginMode = !hasLxappConfig && hasPluginConfig;
  const outputDir = path.join(projectPath, isPluginMode ? 'dist-plugin' : 'dist');

  const buildOptions: BuildOptions = { ...options, release: Boolean(options.release) };

  console.log(`🚀 Starting LingXia ${isPluginMode ? 'plugin' : 'project'} build...`);
  console.log(` Project: ${projectPath}`);
  console.log(` Output: ${outputDir}`);
  console.log(` View bundler: Vite`);

  try {
    const pluginConfig = isPluginMode ? configManager.getLxpluginConfig() : null;
    if (isPluginMode && !pluginConfig) {
      throw new Error(
        'lxplugin.json not found (required for plugin build). Create a lxplugin.json file in the project root.'
      );
    }
    if (isPluginMode && !pluginConfig?.lxPluginId?.trim()) {
      throw new Error('lxplugin.json is missing a valid "lxPluginId".');
    }
    const pluginId = pluginConfig?.lxPluginId?.trim();

    // Validate JSON configuration files
    const jsonFiles = [
      path.join(projectPath, isPluginMode ? 'lxplugin.json' : 'lxapp.json'),
      ...configManager.getPages({ plugin: isPluginMode }).map(p =>
        path.join(projectPath, path.dirname(p), `${path.basename(p, path.extname(p))}.json`)
      )
    ].filter(f => fs.existsSync(f));

    if (!isPluginMode) {
      const lxappConfigPath = path.join(projectPath, 'lxapp.config.json');
      if (!fs.existsSync(lxappConfigPath)) {
        throw new Error('lxapp.config.json not found in project root');
      }
      jsonFiles.push(lxappConfigPath);
    }

    for (const file of jsonFiles) {
      try {
        JSON.parse(fs.readFileSync(file, 'utf-8'));
      } catch (e) {
        throw new Error(`Invalid JSON: ${path.relative(projectPath, file)}\n${e instanceof Error ? e.message : e}`);
      }
    }

    await ensureDependencies(projectPath);

    const buildConfig = !isPluginMode
      ? loadLxappBuildConfig(projectPath)
      : undefined;

    // Clean and prepare output directory
    fileUtils.cleanDirectory(outputDir);

    // Discover pages
    const pages = discoverPages(projectPath, configManager, isPluginMode);
    const pageNames = pages.map(p => p.name).join(', ');
    console.log(` Found ${pages.length} pages: ${pageNames}`);

    if (pages.length === 0) {
      console.warn('⚠️ No pages found in the project');
      return;
    }

    const startTime = Date.now();

    const only = process.env.LINGXIA_ONLY?.toLowerCase();

    if (only === 'logic') {
      console.log('▶ Building logic layer only...');
      const logicBuilder = new LogicBuilder(
        projectPath,
        outputDir,
        pluginId,
        buildConfig
      );
      await logicBuilder.buildLogic(buildOptions);
    } else if (only === 'view') {
      console.log('▶ Building view layer only...');
      const viewBuilder = new ViewBuilder(projectPath, outputDir, buildConfig);
      await viewBuilder.buildPages(pages, buildOptions);
    } else {
      console.log('▶ Building logic and view layers in parallel...');
      const logicBuilder = new LogicBuilder(
        projectPath,
        outputDir,
        pluginId,
        buildConfig
      );
      const viewBuilder = new ViewBuilder(projectPath, outputDir, buildConfig);

      await Promise.all([
        logicBuilder
          .buildLogic(buildOptions)
          .then(() => console.log('  ✔ Logic layer built')),
        viewBuilder
          .buildPages(pages, buildOptions)
          .then(() => console.log('  ✔ View layer built')),
      ]);
    }

    const endTime = Date.now();
    const buildTime = ((endTime - startTime) / 1000).toFixed(2);

    cleanupLingxiaBuild(projectPath);

    // Copy configuration file to output
    if (pluginConfig) {
      const pluginConfigSrc = path.join(projectPath, 'lxplugin.json');
      const pluginConfigDest = path.join(outputDir, 'lxplugin.json');
      fs.copyFileSync(pluginConfigSrc, pluginConfigDest);
      console.log('  ✔ Copied lxplugin.json to output');
    }

    const packageInfo = readPackageInfo(projectPath);
    const packagePath = await packageDist(
      outputDir,
      projectPath,
      packageInfo,
      isPluginMode
    );
    const relativePackagePath =
      path.relative(projectPath, packagePath) || packagePath;

    console.log('Build completed successfully!');
    console.log(` Completed in ${buildTime}s`);
    console.log(` Output directory: ${outputDir}`);
    console.log(` Package: ${relativePackagePath}`);

  } catch (error) {
    console.error(
      '❌ Build failed:',
      error instanceof Error ? error.message : String(error)
    );
    process.exit(1);
  }
}

function discoverPages(
  projectPath: string,
  configManager: ConfigManager,
  isPluginMode: boolean
): Page[] {
  const pagesPaths = configManager.getPages({ plugin: isPluginMode });

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

type PackageManager = 'npm' | 'pnpm' | 'yarn';

async function ensureDependencies(projectPath: string): Promise<void> {
  if (isSkipInstall()) return;

  const packageJson = path.join(projectPath, 'package.json');
  if (!fs.existsSync(packageJson)) return;

  const nodeModules = path.join(projectPath, 'node_modules');
  const nodeModulesMtime = getMtime(nodeModules);
  const packageMtime = getMtime(packageJson);

  const pnpmLock = path.join(projectPath, 'pnpm-lock.yaml');
  const yarnLock = path.join(projectPath, 'yarn.lock');
  const npmLock = path.join(projectPath, 'package-lock.json');

  const hasPnpmLock = fs.existsSync(pnpmLock);
  const hasYarnLock = fs.existsSync(yarnLock);
  const hasNpmLock = fs.existsSync(npmLock);

  const newestLockMtime = Math.max(
    getMtime(pnpmLock),
    getMtime(yarnLock),
    getMtime(npmLock)
  );

  const shouldInstall =
    !fs.existsSync(nodeModules) ||
    newestLockMtime > nodeModulesMtime ||
    packageMtime > nodeModulesMtime;

  if (!shouldInstall) return;

  const packageManager = detectPackageManager(hasPnpmLock, hasYarnLock);
  const args = resolveInstallArgs(
    packageManager,
    hasLockfileFor(packageManager, hasPnpmLock, hasYarnLock, hasNpmLock)
  );

  console.log(`  ⏳ Installing LxApp dependencies (${packageManager})...`);

  try {
    await runCommand(packageManager, args, projectPath);
  } catch (error: any) {
    if (error?.code === 'ENOENT' && packageManager !== 'npm') {
      console.warn(`⚠️ ${packageManager} not found, falling back to npm install.`);
      await runCommand('npm', resolveInstallArgs('npm', hasNpmLock), projectPath);
      return;
    }
    throw error;
  }
}

function isSkipInstall(): boolean {
  const value = process.env.LINGXIA_SKIP_NPM_INSTALL;
  if (!value) return false;
  return value === '1' || value.toLowerCase() === 'true';
}

function getMtime(targetPath: string): number {
  try {
    return fs.statSync(targetPath).mtimeMs || 0;
  } catch {
    return 0;
  }
}

function detectPackageManager(hasPnpmLock: boolean, hasYarnLock: boolean): PackageManager {
  if (hasPnpmLock) return 'pnpm';
  if (hasYarnLock) return 'yarn';
  return 'npm';
}

function hasLockfileFor(
  manager: PackageManager,
  hasPnpmLock: boolean,
  hasYarnLock: boolean,
  hasNpmLock: boolean
): boolean {
  if (manager === 'pnpm') return hasPnpmLock;
  if (manager === 'yarn') return hasYarnLock;
  return hasNpmLock;
}

function resolveInstallArgs(manager: PackageManager, hasLock: boolean): string[] {
  const isCi = Boolean(process.env.CI);
  if (manager === 'npm') {
    return isCi && hasLock ? ['ci'] : ['install'];
  }
  if (manager === 'pnpm') {
    return isCi && hasLock ? ['install', '--frozen-lockfile'] : ['install'];
  }
  return isCi && hasLock ? ['install', '--frozen-lockfile'] : ['install'];
}

function runCommand(command: string, args: string[], cwd: string): Promise<void> {
  return new Promise((resolve, reject) => {
    const child = spawn(command, args, { cwd, stdio: 'inherit' });
    child.on('error', err => reject(err));
    child.on('exit', code => {
      if (code === 0) {
        resolve();
      } else {
        const error = new Error(`${command} exited with code ${code ?? 'unknown'}`);
        (error as any).code = code;
        reject(error);
      }
    });
  });
}

async function packageDist(
  distDir: string,
  projectPath: string,
  pkgInfo: PackageInfo,
  isPluginMode: boolean = false
): Promise<string> {
  if (!fs.existsSync(distDir)) {
    throw new Error('Dist directory not found, cannot package build output.');
  }

  const defaultName = isPluginMode ? 'lingxia-plugin' : 'lingxia-app';
  const baseName = sanitizeName(pkgInfo.name, defaultName);
  const version = sanitizeVersion(pkgInfo.version);
  const archiveName = `${baseName}-${version}.tar.zstd`;
  const archivePath = path.join(projectPath, archiveName);

  if (fs.existsSync(archivePath)) {
    fs.rmSync(archivePath, { force: true });
  }

  // Package from inside the dist directory so extracted files don't have dist/ prefix
  // Exclude macOS metadata files (._* and .DS_Store) and other hidden files
  // Note: --exclude must come before -cf for proper filtering
  await runTar(
    [
      '--exclude=._*',
      '--exclude=.DS_Store',
      '--use-compress-program',
      'zstd -T1',
      '-cf',
      archivePath,
      '.'
    ],
    distDir
  );
  return archivePath;
}

function sanitizeName(name: unknown, fallback: string): string {
  if (!name || typeof name !== 'string') {
    return fallback;
  }
  const cleaned = name.trim().replace(/[^a-zA-Z0-9._-]/g, '_');
  return cleaned.length > 0 ? cleaned : fallback;
}

function sanitizeVersion(version: unknown): string {
  const fallback = '0.0.0';
  if (!version || typeof version !== 'string') {
    return fallback;
  }
  const cleaned = version.trim().replace(/[^0-9a-zA-Z._-]/g, '_');
  return cleaned.length > 0 ? cleaned : fallback;
}

function runTar(args: string[], cwd: string): Promise<void> {
  return new Promise((resolve, reject) => {
    // COPYFILE_DISABLE=1 prevents macOS tar from adding ._* metadata files
    const child = spawn('tar', args, {
      cwd,
      stdio: 'inherit',
      env: {
        ...process.env,
        COPYFILE_DISABLE: '1',
        ZSTD_NBTHREADS: '1',
        ZSTD_DEFAULT_NBTHREADS: '1'
      }
    });
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
