"use strict";

const fs = require("node:fs");
const path = require("node:path");

function readJson(filePath) {
  return JSON.parse(fs.readFileSync(filePath, "utf8"));
}

function writeJson(filePath, value) {
  fs.writeFileSync(filePath, JSON.stringify(value, null, 2) + "\n", "utf8");
}

function main() {
  const version = process.argv[2];
  if (!version) {
    console.error("Usage: node set-version.cjs <version>");
    process.exit(2);
  }

  const cliPackageJson = path.resolve(__dirname, "..", "package.json");
  const cliPkg = readJson(cliPackageJson);
  cliPkg.version = version;
  if (cliPkg.optionalDependencies) {
    for (const [name] of Object.entries(cliPkg.optionalDependencies)) {
      if (name.startsWith("@lingxia/cli-")) {
        cliPkg.optionalDependencies[name] = version;
      }
    }
  }
  writeJson(cliPackageJson, cliPkg);
}

main();
