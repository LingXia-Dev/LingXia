import fs from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { rolldown } from "rolldown";
import ts from "typescript";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const packageDir = path.resolve(__dirname, "..");
const bridgeModules = path.resolve(packageDir, "../lingxia-bridge/dist/es2020");
const distDir = path.join(packageDir, "dist");
const bridgeFacadeFile = path.join(distDir, "__bridge_facade__.js");
const bundleFile = path.join(distDir, "global.bundle.js");
const modernFile = path.join(distDir, "global.es2020.js");
const legacyFile = path.join(distDir, "global.es5.js");

// The host already booted the bridge; importing its index would boot another.
// The page API needs only the fault panel and the host store.
await fs.writeFile(
  bridgeFacadeFile,
  [
    `export { renderPageFault } from ${JSON.stringify(path.join(bridgeModules, "error.js"))};`,
    `export { getHost, subscribeHost } from ${JSON.stringify(path.join(bridgeModules, "host.js"))};`,
    `export { setLeaveGuard, subscribeBackRequest } from ${JSON.stringify(path.join(bridgeModules, "leave-guard.js"))};`,
    "",
  ].join("\n"),
);
try {
  const bundle = await rolldown({
    input: path.join(distDir, "global.js"),
    resolve: { alias: { "@lingxia/bridge": bridgeFacadeFile } },
  });
  await bundle.write({ file: bundleFile, format: "iife", name: "LingXiaPage" });
  await bundle.close();
  await fs.copyFile(bundleFile, modernFile);
  await writeLegacyBundle(bundleFile, legacyFile);
  await stripSourceMap(modernFile);
  await stripSourceMap(legacyFile);
} finally {
  await fs.rm(bundleFile, { force: true });
  await fs.rm(bridgeFacadeFile, { force: true });
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
