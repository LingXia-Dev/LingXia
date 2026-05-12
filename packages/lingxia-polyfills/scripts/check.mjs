// Run dist/polyfills.es5.js in a vm sandbox that simulates Chromium 37
// (Android 5.0 stock WebView), then assert that each polyfill we promise
// actually installed itself. If a feature detect ever gets inverted (e.g.
// via a terser regression — see src/es5.js) this check fails loudly.

import vm from "node:vm";
import fs from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const bundlePath = path.resolve(__dirname, "..", "dist", "polyfills.es5.js");

// Globals that we delete from the sandbox before loading the polyfill. The
// polyfill must install replacements for all of them.
const POLYFILLED_GLOBALS = ["Symbol", "Set", "Map"];
const POLYFILLED_STATICS = [
  "Object.assign",
  "Object.entries",
  "Object.values",
  "Object.fromEntries",
  "Array.from",
  "Array.prototype.includes",
  "Array.prototype.find",
  "Array.prototype.findIndex",
  "Array.prototype.flat",
  "Array.prototype.flatMap",
  "Array.prototype.at",
  "String.prototype.startsWith",
  "String.prototype.endsWith",
  "String.prototype.includes",
  "String.prototype.padStart",
  "String.prototype.padEnd",
  "String.prototype.trimStart",
  "String.prototype.trimEnd",
  "String.prototype.replaceAll",
  "Number.isFinite",
  "Number.isInteger",
  "Promise.prototype.finally",
  "Promise.allSettled",
  "Promise.any",
];
// Members the polyfill installs as non-function references (typeof === "object").
const POLYFILLED_OBJECT_REFS = ["globalThis"];

const source = await fs.readFile(bundlePath, "utf8").catch((err) => {
  console.error(`[polyfills:check] cannot read ${bundlePath}`);
  console.error(`[polyfills:check] run \`npm run build\` first.`);
  throw err;
});

const sandbox = {
  console,
  // Stub just enough DOM for the polyfill's no-op branches not to throw.
  document: { addEventListener() {}, readyState: "loading" },
  navigator: { userAgent: "lingxia-polyfills-check" },
  location: { href: "about:blank" },
};
sandbox.window = sandbox;
sandbox.globalThis = sandbox;
sandbox.self = sandbox;

vm.createContext(sandbox);

// Node's vm realm ships its own copies of every JS builtin. To emulate
// Chromium 37, delete them inside the context.
const deleteStmts =
  POLYFILLED_GLOBALS.map((name) => `delete globalThis.${name};`).join(" ") +
  POLYFILLED_STATICS.map((p) => ` delete ${p};`).join("") +
  POLYFILLED_OBJECT_REFS.map((name) => ` delete globalThis.${name};`).join("");
new vm.Script(deleteStmts).runInContext(sandbox);

try {
  new vm.Script(source, { filename: "polyfills.es5.js" }).runInContext(sandbox);
} catch (err) {
  console.error(`[polyfills:check] FAIL: polyfill bundle threw at load:`);
  console.error(err);
  process.exit(1);
}

// Probe every promised member.
const expectedType = {};
POLYFILLED_GLOBALS.forEach((n) => (expectedType[n] = "function"));
POLYFILLED_STATICS.forEach((n) => (expectedType[n] = "function"));
POLYFILLED_OBJECT_REFS.forEach((n) => (expectedType[n] = "object"));
const probeExpr =
  "JSON.stringify({" +
  Object.keys(expectedType)
    .map((name) => `${JSON.stringify(name)}: typeof ${name}`)
    .join(", ") +
  "})";
const probe = JSON.parse(new vm.Script(probeExpr).runInContext(sandbox));
const missing = Object.entries(probe).filter(
  ([name, actual]) => actual !== expectedType[name],
);
if (missing.length > 0) {
  console.error(
    `[polyfills:check] FAIL: not installed: ${missing
      .map(([n, t]) => `${n} (typeof=${t}, expected=${expectedType[n]})`)
      .join(", ")}`,
  );
  process.exit(1);
}

console.log(
  `[polyfills:check] OK: ${path.basename(bundlePath)} installs ${
    Object.keys(expectedType).length
  } members on an emulated Chromium 37 sandbox.`,
);
