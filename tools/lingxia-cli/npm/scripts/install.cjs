#!/usr/bin/env node
"use strict";

const fs = require("node:fs");
const path = require("node:path");
const { createRequire } = require("node:module");

function resolvePlatformPackage() {
  const platform = process.platform;
  const arch = process.arch;

  if (platform === "darwin" && arch === "arm64") return "@lingxia/cli-darwin-arm64";
  if (platform === "darwin" && arch === "x64") return "@lingxia/cli-darwin-x64";

  return null;
}

async function main() {
  if (process.env.LINGXIA_CLI_SKIP_DOWNLOAD === "1") return;

  const platformPackage = resolvePlatformPackage();
  if (!platformPackage) {
    throw new Error(
      `Unsupported platform: ${process.platform}-${process.arch}. Supported: darwin-arm64, darwin-x64.`
    );
  }

  const pkgRoot = path.resolve(__dirname, "..");
  const vendorDir = path.join(pkgRoot, "vendor");
  fs.mkdirSync(vendorDir, { recursive: true });

  const binName = process.platform === "win32" ? "lingxia.exe" : "lingxia";
  const dest = path.join(vendorDir, binName);

  if (fs.existsSync(dest)) return;

  const requireFromHere = createRequire(__filename);
  let platformPkgRoot;
  try {
    platformPkgRoot = path.dirname(requireFromHere.resolve(`${platformPackage}/package.json`));
  } catch (err) {
    throw new Error(
      `Platform package ${platformPackage} not found. Ensure optionalDependencies are installed.`
    );
  }

  const candidate = path.join(platformPkgRoot, "bin", binName);
  if (!fs.existsSync(candidate)) {
    throw new Error(`Binary not found in ${platformPackage}: ${candidate}`);
  }

  fs.copyFileSync(candidate, dest);
  if (process.platform !== "win32") {
    fs.chmodSync(dest, 0o755);
  }
}

main().catch((err) => {
  const message = err && typeof err === "object" && "message" in err ? err.message : String(err);
  console.error(`\n@lingxia/cli: install failed: ${message}\n`);
  process.exit(1);
});
