import fs from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { spawn } from "node:child_process";
import ts from "typescript";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const packageDir = path.resolve(__dirname, "..");
const distDir = path.join(packageDir, "dist", "html");
const entryFile = path.join(distDir, "global-entry.js");
const bundleFile = path.join(distDir, "global.bundle.js");
const modernFile = path.join(distDir, "global.js");
const legacyFile = path.join(distDir, "global.es5.js");

await runRolldown();
await fs.copyFile(bundleFile, modernFile);
await writeLegacyBundle();

try {
  await stripSourceMap(modernFile);
  await stripSourceMap(legacyFile);
} finally {
  await fs.rm(bundleFile, { force: true });
}

function runRolldown() {
  return runCommand(
    "npx",
    ["rolldown", entryFile, "--file", bundleFile, "--format", "iife", "--name", "LingXiaPage"],
    packageDir,
    "rolldown",
  );
}

function runCommand(command, args, cwd, label) {
  return new Promise((resolve, reject) => {
    const child = spawn(command, args, {
      cwd,
      stdio: "inherit",
      env: process.env,
    });

    child.on("exit", (code) => {
      if (code === 0) {
        resolve();
      } else {
        reject(new Error(`${label} failed with exit code ${code ?? "unknown"}`));
      }
    });
    child.on("error", reject);
  });
}

async function writeLegacyBundle() {
  const source = await fs.readFile(bundleFile, "utf8");
  const result = ts.transpileModule(source, {
    compilerOptions: {
      target: ts.ScriptTarget.ES5,
      module: ts.ModuleKind.None,
      removeComments: false,
    },
  });
  await fs.writeFile(legacyFile, result.outputText, "utf8");
}

async function stripSourceMap(file) {
  const source = await fs.readFile(file, "utf8");
  await fs.writeFile(file, source.replace(/\/\/# sourceMappingURL=.*\n?$/, ""));
}
