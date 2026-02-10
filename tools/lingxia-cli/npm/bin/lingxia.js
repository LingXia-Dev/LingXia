#!/usr/bin/env node

import { spawn, spawnSync } from "child_process";
import { existsSync, readFileSync, writeFileSync, mkdirSync } from "fs";
import { createRequire } from "module";
import { dirname, resolve } from "path";
import { fileURLToPath, pathToFileURL } from "url";
import { homedir } from "os";
import { createInterface } from "readline";

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);
const binName = process.platform === "win32" ? "lingxia.exe" : "lingxia";
const require = createRequire(import.meta.url);
const pkg = require("../package.json");
const jsCli = resolve(__dirname, "../dist/index.js");

// Cross-platform config directory
function getConfigDir() {
  if (process.platform === "win32") {
    // Windows: use APPDATA
    return resolve(
      process.env.APPDATA || resolve(homedir(), "AppData", "Roaming"),
      "lingxia",
    );
  }
  // Linux/macOS: prefer XDG_CONFIG_HOME, fallback to ~/.config
  const xdgConfig =
    process.env.XDG_CONFIG_HOME || resolve(homedir(), ".config");
  return resolve(xdgConfig, "lingxia");
}

// Simple version check with cache
const CACHE_FILE = resolve(getConfigDir(), "update-check.json");
const CHECK_INTERVAL = 24 * 60 * 60 * 1000; // 24 hours
let updateCheckPromise = null;

// Normalize version string: remove 'v' prefix and pre-release suffixes
function normalizeVersion(version) {
  if (!version) return "0.0.0";
  return version
    .replace(/^v/i, "") // remove 'v' prefix
    .replace(/[-+].*$/, "") // remove -beta.1, +build, etc.
    .trim();
}

function isNewerVersion(current, latest) {
  const cur = normalizeVersion(current)
    .split(".")
    .map((n) => parseInt(n, 10) || 0);
  const lat = normalizeVersion(latest)
    .split(".")
    .map((n) => parseInt(n, 10) || 0);

  for (let i = 0; i < 3; i++) {
    const c = cur[i] || 0;
    const l = lat[i] || 0;
    if (l > c) return true;
    if (l < c) return false;
  }
  return false;
}

function promptUser(question) {
  return new Promise((resolve) => {
    const rl = createInterface({
      input: process.stdin,
      output: process.stdout,
    });
    rl.question(question, (answer) => {
      rl.close();
      resolve(answer.trim().toLowerCase());
    });
  });
}

function performUpdate() {
  console.log("\x1b[36m  Updating @lingxia/cli...\x1b[0m\n");
  const result = spawnSync("npm", ["install", "-g", "@lingxia/cli"], {
    stdio: "inherit",
    shell: true,
  });
  if (result.status === 0) {
    console.log(
      "\n\x1b[32m  Updated successfully! Please re-run your command.\x1b[0m\n",
    );
    process.exit(0);
  } else {
    console.log(
      "\n\x1b[31m  Update failed. Try manually: npm install -g @lingxia/cli\x1b[0m\n",
    );
    process.exit(1);
  }
}

async function notifyUpdateAvailable(latestVersion) {
  // Only prompt if running in interactive terminal
  if (process.stdin.isTTY && process.stdout.isTTY) {
    console.log();
    console.log(
      `\x1b[33m  Update available: ${pkg.version} → ${latestVersion}\x1b[0m`,
    );
    const answer = await promptUser("  Update now? [Y/n] ");
    if (answer === "" || answer === "y" || answer === "yes") {
      performUpdate();
      return;
    }
    console.log();
    return;
  }

  // Non-interactive: just show message
  console.log();
  console.log(
    `\x1b[33m  Update available: ${pkg.version} → ${latestVersion}\x1b[0m`,
  );
  console.log(`\x1b[33m  Run: npm install -g @lingxia/cli\x1b[0m`);
  console.log();
}

async function checkForUpdates() {
  try {
    // Read cache
    let cache = { lastCheck: 0, latestVersion: null };
    try {
      cache = JSON.parse(readFileSync(CACHE_FILE, "utf-8"));
    } catch {}

    const now = Date.now();
    const shouldCheck = now - cache.lastCheck > CHECK_INTERVAL;
    let promptedVersion = null;

    // Prompt for update if cached version is newer
    if (
      cache.latestVersion &&
      isNewerVersion(pkg.version, cache.latestVersion)
    ) {
      promptedVersion = cache.latestVersion;
      await notifyUpdateAvailable(cache.latestVersion);
    }

    // Check registry and prompt immediately if a newer version is discovered now.
    if (shouldCheck) {
      // Use encodeURIComponent for package name (handles @ and /)
      const registryUrl = `https://registry.npmjs.org/${encodeURIComponent(pkg.name)}/latest`;
      updateCheckPromise = fetch(registryUrl, {
        signal: AbortSignal.timeout(3000), // 3 second timeout
      })
        .then((res) => res.json())
        .then((data) => {
          // Validate response before writing cache
          if (data?.version && typeof data.version === "string") {
            mkdirSync(dirname(CACHE_FILE), { recursive: true });
            writeFileSync(
              CACHE_FILE,
              JSON.stringify({ lastCheck: now, latestVersion: data.version }),
            );
            return data.version;
          }
          return null;
        })
        .catch(() => null); // Silently ignore errors

      const latestVersion = await updateCheckPromise;
      if (
        latestVersion &&
        latestVersion !== promptedVersion &&
        isNewerVersion(pkg.version, latestVersion)
      ) {
        await notifyUpdateAvailable(latestVersion);
      }
    }
  } catch {}
}

function findSubcommand(argv) {
  for (const arg of argv) {
    if (!arg.startsWith("-")) return arg;
  }
  return undefined;
}

function isLxappProject(cwd) {
  const hasLxapp = existsSync(resolve(cwd, "lxapp.json"));
  // lxapp.config is optional now (supports .ts, .js, .json)
  const hasHost = existsSync(resolve(cwd, "lingxia.config.json"));
  return hasLxapp && !hasHost;
}

function isLxpluginProject(cwd) {
  const hasPlugin = existsSync(resolve(cwd, "lxplugin.json"));
  const hasHost = existsSync(resolve(cwd, "lingxia.config.json"));
  return hasPlugin && !hasHost;
}

async function runJsCli(argv) {
  const jsCliPath = process.env.LINGXIA_JS_CLI || jsCli;
  if (!existsSync(jsCliPath)) {
    console.error(
      "@lingxia/cli: LxApp JS CLI not found. Run npm install in @lingxia/cli.",
    );
    await exitAfterUpdateCheck(1);
    return;
  }
  const { runCLI } = await import(pathToFileURL(jsCliPath).href);
  await runCLI(argv);
  // Wait for update check to complete before exiting
  await waitForUpdateCheck();
}

// Helper to wait for background update check
async function waitForUpdateCheck() {
  if (updateCheckPromise) {
    await updateCheckPromise.catch(() => {});
  }
}

async function exitAfterUpdateCheck(code) {
  await waitForUpdateCheck();
  process.exit(code);
}

function runRust(argv) {
  const rustBin =
    (process.env.LINGXIA_RUST_CLI && resolve(process.env.LINGXIA_RUST_CLI)) ||
    resolve(__dirname, "../vendor", binName);

  const resolvedRustBin = existsSync(rustBin) ? rustBin : null;

  if (!resolvedRustBin) {
    console.error(
      `@lingxia/cli@${pkg.version}: Rust binary not found. Please reinstall the package.`,
    );
    void exitAfterUpdateCheck(1);
    return;
  }

  if (!process.env.LINGXIA_JS_CLI && existsSync(jsCli)) {
    process.env.LINGXIA_JS_CLI = jsCli;
  }
  if (!process.env.LINGXIA_TEMPLATES_DIR) {
    const templates = resolve(__dirname, "../templates");
    if (existsSync(templates)) {
      process.env.LINGXIA_TEMPLATES_DIR = templates;
    }
  }

  // Use spawn instead of spawnSync to allow background update check to complete
  const child = spawn(resolvedRustBin, argv, {
    stdio: "inherit",
  });

  // Handle spawn errors (binary not executable, permission denied, etc.)
  child.on("error", (err) => {
    console.error(`@lingxia/cli: Failed to start Rust binary: ${err.message}`);
    void exitAfterUpdateCheck(1);
  });

  child.on("close", async (code) => {
    // Wait for update check to complete before exiting
    await waitForUpdateCheck();
    process.exit(code ?? 1);
  });
}

async function main() {
  await checkForUpdates();

  const argv = process.argv.slice(2);
  const subcommand = findSubcommand(argv);
  const lxappProject = isLxappProject(process.cwd());
  const lxpluginProject = isLxpluginProject(process.cwd());

  const useJs = subcommand === "build" && (lxappProject || lxpluginProject);

  if (useJs) {
    try {
      await runJsCli(process.argv);
    } catch (err) {
      console.error(err instanceof Error ? err.message : String(err));
      await exitAfterUpdateCheck(1);
    }
  } else {
    runRust(argv);
  }
}

main();
