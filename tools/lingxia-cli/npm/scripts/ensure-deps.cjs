#!/usr/bin/env node
"use strict";

const fs = require("node:fs");
const path = require("node:path");
const { spawnSync } = require("node:child_process");

function hasDependency(pkgRoot, name) {
  return fs.existsSync(path.join(pkgRoot, "node_modules", name));
}

function runNpmCi(pkgRoot) {
  const nodeExec = process.env.npm_node_execpath || process.execPath;
  const npmExecPath = process.env.npm_execpath;

  let result;
  if (npmExecPath) {
    result = spawnSync(nodeExec, [npmExecPath, "ci"], {
      cwd: pkgRoot,
      stdio: "inherit",
      env: process.env,
    });
  } else {
    const npmCmd = process.platform === "win32" ? "npm.cmd" : "npm";
    result = spawnSync(npmCmd, ["ci"], {
      cwd: pkgRoot,
      stdio: "inherit",
      env: process.env,
    });
  }

  if (result.status !== 0) {
    throw new Error("Failed to install dependencies (npm ci)");
  }
}

function main() {
  const pkgRoot = path.resolve(__dirname, "..");
  const needTypescript = !hasDependency(pkgRoot, "typescript");
  const needNodeTypes = !hasDependency(pkgRoot, "@types/node");

  if (!needTypescript && !needNodeTypes) return;

  console.log("@lingxia/cli: dependencies missing, running npm ci...");
  runNpmCi(pkgRoot);
}

try {
  main();
} catch (err) {
  const message = err && typeof err === "object" && "message" in err ? err.message : String(err);
  console.error(`\n@lingxia/cli: ensure deps failed: ${message}\n`);
  process.exit(1);
}
