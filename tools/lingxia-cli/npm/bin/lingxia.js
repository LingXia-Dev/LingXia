#!/usr/bin/env node

import { spawnSync } from "child_process";
import { existsSync } from "fs";
import { createRequire } from "module";
import { dirname, resolve } from "path";
import { fileURLToPath, pathToFileURL } from "url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);
const binName = process.platform === "win32" ? "lingxia.exe" : "lingxia";
const require = createRequire(import.meta.url);
const pkg = require("../package.json");
const jsCli = resolve(__dirname, "../dist/index.js");

function findSubcommand(argv) {
  for (const arg of argv) {
    if (!arg.startsWith("-")) return arg;
  }
  return undefined;
}

function isLxappProject(cwd) {
  const hasLxapp = existsSync(resolve(cwd, "lxapp.json"));
  const hasLxappConfig = existsSync(resolve(cwd, "lxapp.config.json"));
  const hasHost = existsSync(resolve(cwd, "lingxia.config.json"));
  return hasLxapp && hasLxappConfig && !hasHost;
}

async function runJsCli(argv) {
  const jsCliPath = process.env.LINGXIA_JS_CLI || jsCli;
  if (!existsSync(jsCliPath)) {
    console.error(
      "@lingxia/cli: LxApp JS CLI not found. Run npm install in @lingxia/cli.",
    );
    process.exit(1);
  }
  const { runCLI } = await import(pathToFileURL(jsCliPath).href);
  await runCLI(argv);
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
    process.exit(1);
  }

  if (!process.env.LINGXIA_JS_CLI && existsSync(jsCli)) {
    process.env.LINGXIA_JS_CLI = jsCli;
  }
  if (!process.env.LINGXIA_TEMPLATES_DIR) {
    const candidates = [
      resolve(__dirname, "../templates"),
      resolve(__dirname, "../../templates"),
    ];
    for (const dir of candidates) {
      if (existsSync(dir)) {
        process.env.LINGXIA_TEMPLATES_DIR = dir;
        break;
      }
    }
  }

  const result = spawnSync(resolvedRustBin, argv, {
    stdio: "inherit",
  });
  process.exit(result.status ?? 1);
}

const argv = process.argv.slice(2);
const subcommand = findSubcommand(argv);
const lxappProject = isLxappProject(process.cwd());

const useJs = subcommand === "build" && lxappProject;

if (useJs) {
  runJsCli(process.argv).catch((err) => {
    console.error(err instanceof Error ? err.message : String(err));
    process.exit(1);
  });
} else {
  runRust(argv);
}
