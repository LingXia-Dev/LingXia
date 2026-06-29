import fs from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { spawn } from "node:child_process";
import ts from "typescript";
import { resolveBin } from "./resolve-bin.mjs";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const packageDir = path.resolve(__dirname, "..");
const pageRuntimeDir = path.resolve(packageDir, "../lingxia-page-runtime");
const bridgeDir = path.resolve(packageDir, "../lingxia-bridge");
const distDir = path.join(packageDir, "dist");
const entryFile = path.join(distDir, "__entry__.js");
const runtimeShimFile = path.join(distDir, "__page_runtime_runtime__.js");
const bundleFile = path.join(distDir, "global.bundle.js");
const modernFile = path.join(distDir, "global.es2020.js");
const legacyFile = path.join(distDir, "global.es5.js");
const rolldownBin = resolveBin(packageDir, "rolldown");

await writeEntryFile();
await runRolldown(entryFile, bundleFile, "iife", "LingXiaPage");
await fs.copyFile(bundleFile, modernFile);
await writeLegacyBundle(bundleFile, legacyFile);

try {
  await stripSourceMap(modernFile);
  await stripSourceMap(legacyFile);
} finally {
  await fs.rm(bundleFile, { force: true });
  await fs.rm(entryFile, { force: true });
  await fs.rm(runtimeShimFile, { force: true });
}

async function writeEntryFile() {
  await writeRuntimeShimFile();
  const importPath = normalizeImportPath(path.relative(distDir, runtimeShimFile));
  const source = [
    'export {',
    '  getPageActions as getActions,',
    '  getPageSnapshot as getSnapshot,',
    '  getPageStateInfo as getStateInfo,',
    '  subscribePageData as subscribe,',
    '  subscribePageSnapshot as subscribeSnapshot,',
    `} from "${importPath}";`,
    "",
  ].join("\n");
  await fs.writeFile(entryFile, source, "utf8");
}

async function writeRuntimeShimFile() {
  const pageRuntimeFile = path.join(pageRuntimeDir, "dist", "shared", "runtime.js");
  const bridgeModuleFile = path.join(bridgeDir, "dist", "es2020", "index.js");
  const bridgeImportPath = normalizeImportPath(path.relative(distDir, bridgeModuleFile));
  const source = await fs.readFile(pageRuntimeFile, "utf8");
  const rewritten = source.replaceAll('"@lingxia/bridge"', `"${bridgeImportPath}"`);
  await fs.writeFile(runtimeShimFile, rewritten, "utf8");
}

function runRolldown(inputFile, outputFile, format, globalName) {
  const args = [inputFile, "--file", outputFile, "--format", format];
  if (globalName) {
    args.push("--name", globalName);
  }
  return new Promise((resolve, reject) => {
    const child = spawn(rolldownBin, args, {
      cwd: packageDir,
      stdio: "inherit",
      // On Windows the .bin entry is rolldown.cmd; route through the shell so
      // it resolves (node's spawn can't exec a .cmd directly -> spawn EINVAL).
      shell: process.platform === "win32",
      env: process.env,
    });

    child.on("exit", (code) => {
      if (code === 0) {
        resolve();
      } else {
        reject(new Error(`rolldown failed with exit code ${code ?? "unknown"}`));
      }
    });
    child.on("error", reject);
  });
}

async function writeLegacyBundle(sourceFile, outputFile) {
  const source = await fs.readFile(sourceFile, "utf8");
  const result = ts.transpileModule(source, {
    compilerOptions: {
      target: ts.ScriptTarget.ES5,
      module: ts.ModuleKind.None,
      removeComments: false,
    },
  });
  await fs.writeFile(outputFile, result.outputText, "utf8");
}

async function stripSourceMap(file) {
  const source = await fs.readFile(file, "utf8");
  await fs.writeFile(file, source.replace(/\/\/# sourceMappingURL=.*\n?$/, ""));
}

function normalizeImportPath(relativePath) {
  const normalized = relativePath.split(path.sep).join("/");
  return normalized.startsWith(".") ? normalized : `./${normalized}`;
}
