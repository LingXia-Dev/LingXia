#!/usr/bin/env node
"use strict";

const fs = require("node:fs");
const path = require("node:path");
const { createRequire } = require("node:module");

const PLATFORM_PACKAGE_MAP = {
  "darwin-arm64": "@lingxia/cli-darwin-arm64",
  "darwin-x64": "@lingxia/cli-darwin-x64",
  "win32-x64": "@lingxia/cli-win32-x64",
};

function resolvePlatformPackage() {
  return PLATFORM_PACKAGE_MAP[`${process.platform}-${process.arch}`] || null;
}

async function main() {
  if (process.env.LINGXIA_CLI_SKIP_DOWNLOAD === "1") return;

  const wrapperPkg = JSON.parse(
    fs.readFileSync(path.resolve(__dirname, "..", "package.json"), "utf8")
  );
  const platformPackage = resolvePlatformPackage();
  if (!platformPackage) {
    const supported = Object.keys(PLATFORM_PACKAGE_MAP).join(", ");
    throw new Error(
      `Unsupported platform: ${process.platform}-${process.arch}. Supported: ${supported}.`
    );
  }

  const pkgRoot = path.resolve(__dirname, "..");
  const vendorDir = path.join(pkgRoot, "vendor");
  fs.mkdirSync(vendorDir, { recursive: true });

  const binName = process.platform === "win32" ? "lingxia.exe" : "lingxia";
  const dest = path.join(vendorDir, binName);

  const requireFromHere = createRequire(__filename);
  let platformPkgRoot;
  try {
    platformPkgRoot = path.dirname(requireFromHere.resolve(`${platformPackage}/package.json`));
  } catch (err) {
    const registry =
      process.env.npm_config_registry || process.env.NPM_CONFIG_REGISTRY || "https://registry.npmjs.org/";
    throw new Error(
      [
        `Platform package ${platformPackage} not found for ${process.platform}-${process.arch}.`,
        `Current npm registry: ${registry}`,
        "Possible causes:",
        "1) Platform package for this version is not published yet.",
        "2) Your mirror/registry is stale or missing @lingxia scoped packages.",
        "Fix: npm config set registry https://registry.npmjs.org/ && npm install -g @lingxia/cli@latest",
      ].join("\n")
    );
  }
  const platformPkg = JSON.parse(
    fs.readFileSync(path.join(platformPkgRoot, "package.json"), "utf8")
  );
  if (platformPkg.version !== wrapperPkg.version) {
    throw new Error(
      [
        `Version mismatch: @lingxia/cli@${wrapperPkg.version} requires ${platformPackage}@${wrapperPkg.version},`,
        `but found ${platformPackage}@${platformPkg.version}.`,
        "Fix: npm install -g @lingxia/cli@latest",
      ].join("\n")
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
