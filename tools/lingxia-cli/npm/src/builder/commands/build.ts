import fs from "fs";
import path from "path";
import { spawn } from "child_process";
import type { BuildOptions, FrameworkType, Page } from "../types/index.js";
import { ViewBuilder } from "../core/builders/view.js";
import { LogicBuilder } from "../core/builders/logic.js";
import { FileUtils } from "../core/utils/file.js";
import { detectPageType, resolvePagePath } from "../core/utils/page.js";
import { ConfigManager } from "../core/config.js";
import { loadLxappConfig } from "../core/config/lxapp-config.js";
import { readProjectFramework } from "../core/config/framework.js";

const fileUtils = new FileUtils();

export async function buildCommand(options: BuildOptions): Promise<void> {
  const projectPath = process.cwd();
  const configManager = new ConfigManager(projectPath);

  const hasLxappConfig = fs.existsSync(path.join(projectPath, "lxapp.json"));
  const hasPluginConfig = fs.existsSync(
    path.join(projectPath, "lxplugin.json"),
  );
  const isPluginMode = !hasLxappConfig && hasPluginConfig;
  const outputDir = path.join(
    projectPath,
    isPluginMode ? "dist-plugin" : "dist",
  );

  // Validate framework option
  const framework = options.framework as FrameworkType | undefined;
  if (framework && framework !== "react" && framework !== "vue") {
    throw new Error(
      `Invalid framework "${framework}". Must be "react" or "vue".`,
    );
  }

  const buildOptions: BuildOptions = {
    ...options,
    release: Boolean(options.release),
    package: Boolean(options.package),
    framework,
  };

  if (buildOptions.package && !buildOptions.release) {
    throw new Error("--package requires --release");
  }

  console.log(
    `🚀 Starting LingXia ${isPluginMode ? "plugin" : "project"} build...`,
  );
  console.log(` Project: ${projectPath}`);
  console.log(` Output: ${outputDir}`);
  console.log(` View bundler: Vite`);

  try {
    const pluginConfig = isPluginMode
      ? configManager.getLxpluginConfig()
      : null;
    if (isPluginMode && !pluginConfig) {
      throw new Error(
        "lxplugin.json not found (required for plugin build). Create a lxplugin.json file in the project root.",
      );
    }
    if (isPluginMode && !pluginConfig?.lxPluginId?.trim()) {
      throw new Error('lxplugin.json is missing a valid "lxPluginId".');
    }
    const pluginId = pluginConfig?.lxPluginId?.trim();

    // Validate JSON configuration files
    const jsonFiles = [
      path.join(projectPath, isPluginMode ? "lxplugin.json" : "lxapp.json"),
      ...configManager
        .getPages({ plugin: isPluginMode })
        .map((p) =>
          path.join(
            projectPath,
            path.dirname(p),
            `${path.basename(p, path.extname(p))}.json`,
          ),
        ),
    ].filter((f) => fs.existsSync(f));

    if (!isPluginMode) {
      const lxappConfigPath = path.join(projectPath, "lxapp.config.ts");
      if (!fs.existsSync(lxappConfigPath)) {
        throw new Error("lxapp.config.ts not found in project root");
      }
    }

    for (const file of jsonFiles) {
      try {
        JSON.parse(fs.readFileSync(file, "utf-8"));
      } catch (e) {
        throw new Error(
          `Invalid JSON: ${path.relative(projectPath, file)}\n${e instanceof Error ? e.message : e}`,
        );
      }
    }

    await ensureDependencies(projectPath);
    await ensureLocalFileDependenciesBuilt(projectPath);

    const buildConfig = !isPluginMode
      ? loadLxappConfig(projectPath)
      : undefined;

    // Clean and prepare output directory
    fileUtils.cleanDirectory(outputDir);

    // Discover pages
    const frameworkForPages = resolveFrameworkPreference(
      projectPath,
      framework,
    );
    const pages = discoverPages(
      projectPath,
      configManager,
      isPluginMode,
      frameworkForPages,
    );
    const logicEntry = isPluginMode ? "logic.js" : configManager.getLogicEntry();
    const logicEnabled = logicEntry !== null;
    const pageNames = pages.map((p) => p.name).join(", ");
    const detectedFramework = pages[0]?.type ?? "unknown";
    console.log(` Found ${pages.length} pages: ${pageNames}`);
    if (framework) {
      console.log(` Framework: ${framework} (specified)`);
    } else {
      console.log(` Framework: ${detectedFramework} (auto-detected)`);
    }

    if (pages.length === 0) {
      console.warn("⚠️ No pages found in the project");
      return;
    }

    const startTime = Date.now();

    const only = process.env.LINGXIA_ONLY?.toLowerCase();

    // Extract resolved page paths for logic builder
    const resolvedPagePaths = pages.map((p) => p.path);

    if (only === "logic") {
      if (!logicEnabled) {
        console.log("▶ Logic disabled in lxapp.json; skipping logic layer.");
      } else {
        console.log("▶ Building logic layer only...");
        const logicBuilder = new LogicBuilder(
          projectPath,
          outputDir,
          pluginId,
          buildConfig,
        );
        await logicBuilder.buildLogic(buildOptions, resolvedPagePaths);
      }
    } else if (only === "view") {
      console.log("▶ Building view layer only...");
      const viewBuilder = new ViewBuilder(
        projectPath,
        outputDir,
        buildConfig,
        framework,
      );
      await viewBuilder.buildPages(pages, buildOptions);
    } else {
      console.log(
        logicEnabled
          ? "▶ Building logic and view layers in parallel..."
          : "▶ Building view layer only (logic disabled)...",
      );
      const viewBuilder = new ViewBuilder(
        projectPath,
        outputDir,
        buildConfig,
        framework,
      );
      const tasks: Array<Promise<void>> = [
        viewBuilder
          .buildPages(pages, buildOptions)
          .then(() => console.log("  ✔ View layer built")),
      ];
      if (logicEnabled) {
        const logicBuilder = new LogicBuilder(
          projectPath,
          outputDir,
          pluginId,
          buildConfig,
        );
        tasks.push(
          logicBuilder
            .buildLogic(buildOptions, resolvedPagePaths)
            .then(() => console.log("  ✔ Logic layer built")),
        );
      }
      await Promise.all(tasks);
    }

    const endTime = Date.now();
    const buildTime = ((endTime - startTime) / 1000).toFixed(2);

    cleanupLingxiaBuild(projectPath);

    // Copy configuration file to output
    if (pluginConfig) {
      const pluginConfigSrc = path.join(projectPath, "lxplugin.json");
      const pluginConfigDest = path.join(outputDir, "lxplugin.json");
      fs.copyFileSync(pluginConfigSrc, pluginConfigDest);
      console.log("  ✔ Copied lxplugin.json to output");
    }

    console.log("Build completed successfully!");
    console.log(` Completed in ${buildTime}s`);
    console.log(` Output directory: ${outputDir}`);
    if (buildOptions.package) {
      const packageInfo = resolvePackageInfo(
        projectPath,
        configManager,
        isPluginMode,
        pluginConfig,
      );
      const packagePath = await packageDist(
        outputDir,
        projectPath,
        packageInfo,
        isPluginMode,
      );
      const relativePackagePath =
        path.relative(projectPath, packagePath) || packagePath;
      console.log(` Package: ${relativePackagePath}`);
    } else {
      console.log(" Package: skipped (use --package to enable)");
    }
  } catch (error) {
    console.error(
      "❌ Build failed:",
      error instanceof Error ? error.message : String(error),
    );
    process.exit(1);
  }
}

function discoverPages(
  projectPath: string,
  configManager: ConfigManager,
  isPluginMode: boolean,
  framework?: FrameworkType,
): Page[] {
  const pagesPaths = configManager.getPages({ plugin: isPluginMode });

  const pages: Page[] = [];

  for (const pagePath of pagesPaths) {
    // Resolve page path (handles both with and without extension)
    const resolvedPath = resolvePagePath(projectPath, pagePath, framework);

    if (!resolvedPath) {
      console.warn(`⚠️ Page file not found: ${pagePath}`);
      continue;
    }

    // Extract page info
    const pageType = detectPageType(resolvedPath);
    const pageDir = path.dirname(resolvedPath);
    const baseName = path.basename(resolvedPath, path.extname(resolvedPath));

    // Create page name from directory structure
    let pageName = pageDir;
    if (pageDir.startsWith("pages/")) {
      pageName = pageDir.substring(6); // Remove 'pages/' prefix
    }
    if (!pageName) {
      pageName = baseName;
    }

    pages.push({
      path: resolvedPath, // Resolved path with extension
      name: pageName,
      type: pageType,
    });
  }

  return pages;
}

function resolveFrameworkPreference(
  projectPath: string,
  framework?: FrameworkType,
): FrameworkType | undefined {
  if (framework) return framework;
  try {
    return readProjectFramework(projectPath);
  } catch {
    return undefined;
  }
}

function cleanupLingxiaBuild(projectPath: string): void {
  const tempDir = path.join(projectPath, ".lingxia", "build");
  if (fs.existsSync(tempDir)) {
    fs.rmSync(tempDir, { recursive: true, force: true });
  }
}

type PackageInfo = {
  name?: string;
  version?: string;
};

function resolvePackageInfo(
  projectPath: string,
  configManager: ConfigManager,
  isPluginMode: boolean,
  pluginConfig: { lxPluginId: string; version: string } | null,
): PackageInfo {
  const packageJson = readOptionalJsonFile(
    path.join(projectPath, "package.json"),
  );
  const rawPackageName = packageJson?.name;
  const packageName =
    typeof rawPackageName === "string" && rawPackageName.trim().length > 0
      ? rawPackageName.trim()
      : undefined;
  const rawPackageVersion = packageJson?.version;
  const packageVersion =
    typeof rawPackageVersion === "string" && rawPackageVersion.trim().length > 0
      ? rawPackageVersion.trim()
      : undefined;

  if (isPluginMode) {
    const manifestVersion = pluginConfig?.version?.trim();
    if (!manifestVersion) {
      throw new Error('lxplugin.json is missing a valid "version".');
    }
    if (packageVersion && packageVersion !== manifestVersion) {
      throw new Error(
        `Version mismatch: lxplugin.json version "${manifestVersion}" does not match package.json version "${packageVersion}".`,
      );
    }

    return {
      name: packageName ?? pluginConfig?.lxPluginId,
      version: manifestVersion,
    };
  }

  const lxappConfig = configManager.getLxappConfig();
  const rawManifestVersion = lxappConfig.version;
  const manifestVersion =
    typeof rawManifestVersion === "string" &&
    rawManifestVersion.trim().length > 0
      ? rawManifestVersion.trim()
      : undefined;
  if (!manifestVersion) {
    throw new Error('lxapp.json is missing a valid "version".');
  }
  if (packageVersion && packageVersion !== manifestVersion) {
    throw new Error(
      `Version mismatch: lxapp.json version "${manifestVersion}" does not match package.json version "${packageVersion}".`,
    );
  }

  return {
    name: packageName,
    version: manifestVersion,
  };
}

function readOptionalJsonFile(
  filePath: string,
): { name?: unknown; version?: unknown } | null {
  if (!fs.existsSync(filePath)) {
    return null;
  }

  try {
    const raw = fs.readFileSync(filePath, "utf-8");
    return JSON.parse(raw) as { name?: unknown; version?: unknown };
  } catch (error) {
    console.warn(
      `⚠️ Failed to read ${path.basename(filePath)}:`,
      error instanceof Error ? error.message : String(error),
    );
    return null;
  }
}

type PackageManager = "npm" | "pnpm" | "yarn";

type PackageJsonLike = {
  name?: unknown;
  version?: unknown;
  main?: unknown;
  types?: unknown;
  exports?: unknown;
  scripts?: Record<string, unknown>;
  dependencies?: Record<string, unknown>;
  devDependencies?: Record<string, unknown>;
  peerDependencies?: Record<string, unknown>;
  optionalDependencies?: Record<string, unknown>;
};

async function ensureDependencies(projectPath: string): Promise<void> {
  if (isSkipInstall()) return;

  const packageJson = path.join(projectPath, "package.json");
  if (!fs.existsSync(packageJson)) return;

  const nodeModules = path.join(projectPath, "node_modules");
  const nodeModulesMtime = getMtime(nodeModules);
  const packageMtime = getMtime(packageJson);

  const pnpmLock = path.join(projectPath, "pnpm-lock.yaml");
  const yarnLock = path.join(projectPath, "yarn.lock");
  const npmLock = path.join(projectPath, "package-lock.json");

  const hasPnpmLock = fs.existsSync(pnpmLock);
  const hasYarnLock = fs.existsSync(yarnLock);
  const hasNpmLock = fs.existsSync(npmLock);

  const newestLockMtime = Math.max(
    getMtime(pnpmLock),
    getMtime(yarnLock),
    getMtime(npmLock),
  );

  const shouldInstall =
    !fs.existsSync(nodeModules) ||
    newestLockMtime > nodeModulesMtime ||
    packageMtime > nodeModulesMtime;

  if (!shouldInstall) return;

  const packageManager = detectPackageManager(hasPnpmLock, hasYarnLock);
  const args = resolveInstallArgs(
    packageManager,
    hasLockfileFor(packageManager, hasPnpmLock, hasYarnLock, hasNpmLock),
  );

  console.log(
    `  ⏳ Installing dependencies for ${path.basename(projectPath)} (${packageManager})...`,
  );

  try {
    await runCommand(packageManager, args, projectPath);
  } catch (error: any) {
    if (error?.code === "ENOENT" && packageManager !== "npm") {
      console.warn(
        `⚠️ ${packageManager} not found, falling back to npm install.`,
      );
      await runCommand(
        "npm",
        resolveInstallArgs("npm", hasNpmLock),
        projectPath,
      );
      return;
    }
    throw error;
  }
}

async function ensureLocalFileDependenciesBuilt(
  projectPath: string,
): Promise<void> {
  const visited = new Set<string>();
  const activeFramework = resolveFrameworkPreference(projectPath);
  await ensureLocalFileDependenciesBuiltInternal(
    projectPath,
    visited,
    true,
    projectPath,
    activeFramework,
  );
}

async function ensureLocalFileDependenciesBuiltInternal(
  projectPath: string,
  visited: Set<string>,
  includeDevDependencies: boolean,
  rootProjectPath: string,
  activeFramework?: FrameworkType,
): Promise<void> {
  const packageJsonPath = path.join(projectPath, "package.json");
  if (!fs.existsSync(packageJsonPath)) return;

  const packageJson = JSON.parse(
    fs.readFileSync(packageJsonPath, "utf-8"),
  ) as PackageJsonLike;
  const localPackages = collectLocalFileDependencyPaths(
    projectPath,
    packageJson,
    includeDevDependencies,
    projectPath === rootProjectPath ? activeFramework : undefined,
  );

  for (const packagePath of localPackages) {
    const normalized = path.resolve(packagePath);
    if (visited.has(normalized)) continue;
    visited.add(normalized);

    await ensureDependencies(normalized);
    await ensureLocalFileDependenciesBuiltInternal(
      normalized,
      visited,
      true,
      rootProjectPath,
      activeFramework,
    );

    const buildArgs = resolveLocalPackageBuildArgs(normalized);
    if (!buildArgs) continue;

    const packageManager = detectPackageManagerForProject(normalized);
    console.log(
      `  ⏳ Building local package ${path.basename(normalized)} (${packageManager})...`,
    );

    try {
      await runCommand(packageManager, buildArgs, normalized);
    } catch (error: any) {
      if (error?.code === "ENOENT" && packageManager !== "npm") {
        console.warn(
          `⚠️ ${packageManager} not found for ${path.basename(normalized)}, falling back to npm run build.`,
        );
        await runCommand("npm", buildArgs, normalized);
        continue;
      }
      throw error;
    }
  }
}

function collectLocalFileDependencyPaths(
  projectPath: string,
  packageJson: PackageJsonLike,
  includeDevDependencies: boolean,
  activeFramework?: FrameworkType,
): string[] {
  const sections: Array<Record<string, unknown> | undefined> = [
    packageJson.dependencies,
    packageJson.optionalDependencies,
  ];
  if (includeDevDependencies) {
    sections.push(packageJson.devDependencies);
  }

  const results: string[] = [];
  const seen = new Set<string>();

  for (const section of sections) {
    if (!section) continue;
    for (const [dependencyName, value] of Object.entries(section)) {
      if (typeof value !== "string" || !value.startsWith("file:")) continue;
      if (
        dependencyName === "lingxia-types" ||
        dependencyName === "@lingxia/types"
      ) {
        continue;
      }
      if (activeFramework === "react" && dependencyName === "@lingxia/vue") {
        continue;
      }
      if (activeFramework === "vue" && dependencyName === "@lingxia/react") {
        continue;
      }
      const rawPath = value.slice("file:".length).trim();
      if (!rawPath) continue;
      const resolved = path.resolve(projectPath, rawPath);
      if (seen.has(resolved)) continue;
      if (!fs.existsSync(path.join(resolved, "package.json"))) continue;
      seen.add(resolved);
      results.push(resolved);
    }
  }

  return results;
}

function resolveLocalPackageBuildArgs(packagePath: string): string[] | null {
  const packageJsonPath = path.join(packagePath, "package.json");
  if (!fs.existsSync(packageJsonPath)) return null;

  const packageJson = JSON.parse(
    fs.readFileSync(packageJsonPath, "utf-8"),
  ) as PackageJsonLike;
  if (!packageJson.scripts?.build) return null;

  const outputs = resolveBuildOutputs(packagePath, packageJson);
  if (outputs.length === 0) return null;

  const newestInput = getNewestRelevantMtime(packagePath, [
    "package.json",
    "package-lock.json",
    "pnpm-lock.yaml",
    "yarn.lock",
    "tsconfig.json",
    "rollup.config.js",
    "rollup.config.mjs",
    "vite.config.ts",
    "vite.config.js",
  ]);
  const newestSource = Math.max(
    newestInput,
    getNewestPathMtime(path.join(packagePath, "src")),
  );
  const affectedOutputs = outputs.filter(
    (output) => !fs.existsSync(output) || newestSource > getMtime(output),
  );
  if (affectedOutputs.length === 0) return null;

  if (
    packageJson.scripts["build:types"] &&
    affectedOutputs.every((output) => output.endsWith(".d.ts"))
  ) {
    return ["run", "build:types"];
  }

  return ["run", "build"];
}

function resolveBuildOutputs(
  packagePath: string,
  packageJson: PackageJsonLike,
): string[] {
  const outputs = new Set<string>();

  const addRelativeOutput = (value: unknown) => {
    if (typeof value !== "string") return;
    if (!value.startsWith("dist/")) return;
    outputs.add(path.join(packagePath, value));
  };

  addRelativeOutput(packageJson.main);
  addRelativeOutput(packageJson.types);

  const visitExports = (value: unknown) => {
    if (typeof value === "string") {
      addRelativeOutput(value.replace(/^\.\//, ""));
      return;
    }
    if (!value || typeof value !== "object") return;
    for (const child of Object.values(value as Record<string, unknown>)) {
      visitExports(child);
    }
  };

  if (
    packageJson.exports &&
    typeof packageJson.exports === "object" &&
    "." in (packageJson.exports as Record<string, unknown>)
  ) {
    visitExports((packageJson.exports as Record<string, unknown>)["."]);
  } else {
    visitExports(packageJson.exports);
  }
  return Array.from(outputs);
}

function getNewestRelevantMtime(projectPath: string, files: string[]): number {
  let newest = 0;
  for (const file of files) {
    newest = Math.max(newest, getMtime(path.join(projectPath, file)));
  }
  return newest;
}

function getNewestPathMtime(targetPath: string): number {
  try {
    const stat = fs.statSync(targetPath);
    if (!stat.isDirectory()) {
      return stat.mtimeMs || 0;
    }

    let newest = stat.mtimeMs || 0;
    for (const entry of fs.readdirSync(targetPath, { withFileTypes: true })) {
      newest = Math.max(
        newest,
        getNewestPathMtime(path.join(targetPath, entry.name)),
      );
    }
    return newest;
  } catch {
    return 0;
  }
}

function isSkipInstall(): boolean {
  const value = process.env.LINGXIA_SKIP_NPM_INSTALL;
  if (!value) return false;
  return value === "1" || value.toLowerCase() === "true";
}

function getMtime(targetPath: string): number {
  try {
    return fs.statSync(targetPath).mtimeMs || 0;
  } catch {
    return 0;
  }
}

function detectPackageManager(
  hasPnpmLock: boolean,
  hasYarnLock: boolean,
): PackageManager {
  if (hasPnpmLock) return "pnpm";
  if (hasYarnLock) return "yarn";
  return "npm";
}

function detectPackageManagerForProject(projectPath: string): PackageManager {
  return detectPackageManager(
    fs.existsSync(path.join(projectPath, "pnpm-lock.yaml")),
    fs.existsSync(path.join(projectPath, "yarn.lock")),
  );
}

function hasLockfileFor(
  manager: PackageManager,
  hasPnpmLock: boolean,
  hasYarnLock: boolean,
  hasNpmLock: boolean,
): boolean {
  if (manager === "pnpm") return hasPnpmLock;
  if (manager === "yarn") return hasYarnLock;
  return hasNpmLock;
}

function resolveInstallArgs(
  manager: PackageManager,
  hasLock: boolean,
): string[] {
  const isCi = Boolean(process.env.CI);
  if (manager === "npm") {
    return isCi && hasLock ? ["ci"] : ["install"];
  }
  if (manager === "pnpm") {
    return isCi && hasLock ? ["install", "--frozen-lockfile"] : ["install"];
  }
  return isCi && hasLock ? ["install", "--frozen-lockfile"] : ["install"];
}

function runCommand(
  command: string,
  args: string[],
  cwd: string,
): Promise<void> {
  return new Promise((resolve, reject) => {
    const child = spawn(command, args, { cwd, stdio: "inherit" });
    child.on("error", (err) => reject(err));
    child.on("exit", (code) => {
      if (code === 0) {
        resolve();
      } else {
        const error = new Error(
          `${command} exited with code ${code ?? "unknown"}`,
        );
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
  isPluginMode: boolean = false,
): Promise<string> {
  if (!fs.existsSync(distDir)) {
    throw new Error("Dist directory not found, cannot package build output.");
  }

  const defaultName = isPluginMode ? "lingxia-plugin" : "lingxia-app";
  const baseName = sanitizeName(pkgInfo.name, defaultName);
  const version = sanitizeVersion(pkgInfo.version);
  const archiveName = `${baseName}-${version}.tar.zst`;
  const archivePath = path.join(projectPath, archiveName);

  if (fs.existsSync(archivePath)) {
    fs.rmSync(archivePath, { force: true });
  }

  // Package from inside the dist directory so extracted files don't have dist/ prefix
  // Exclude macOS metadata files (._* and .DS_Store) and other hidden files
  // Note: --exclude must come before -cf for proper filtering
  await runTar(
    [
      "--exclude=._*",
      "--exclude=.DS_Store",
      "--use-compress-program",
      "zstd -T1",
      "-cf",
      archivePath,
      ".",
    ],
    distDir,
  );
  return archivePath;
}

function sanitizeName(name: unknown, fallback: string): string {
  if (!name || typeof name !== "string") {
    return fallback;
  }
  const cleaned = name.trim().replace(/[^a-zA-Z0-9._-]/g, "_");
  return cleaned.length > 0 ? cleaned : fallback;
}

function sanitizeVersion(version: unknown): string {
  const fallback = "0.0.0";
  if (!version || typeof version !== "string") {
    return fallback;
  }
  const cleaned = version.trim().replace(/[^0-9a-zA-Z._-]/g, "_");
  return cleaned.length > 0 ? cleaned : fallback;
}

function runTar(args: string[], cwd: string): Promise<void> {
  return new Promise((resolve, reject) => {
    // COPYFILE_DISABLE=1 prevents macOS tar from adding ._* metadata files
    const child = spawn("tar", args, {
      cwd,
      stdio: "inherit",
      env: {
        ...process.env,
        COPYFILE_DISABLE: "1",
        ZSTD_NBTHREADS: "1",
        ZSTD_DEFAULT_NBTHREADS: "1",
      },
    });
    child.on("error", (err) => reject(err));
    child.on("exit", (code) => {
      if (code === 0) {
        resolve();
      } else {
        reject(new Error(`tar exited with code ${code ?? "unknown"}`));
      }
    });
  });
}
