import { spawnSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import ts from "typescript";

const packageDir = resolve(dirname(fileURLToPath(import.meta.url)), "..");

function declarations(path) {
  const source = ts.createSourceFile(
    path,
    readFileSync(path, "utf8"),
    ts.ScriptTarget.Latest,
    true,
    ts.ScriptKind.TS,
  );
  const names = new Set();
  const documented = new Set();
  function isExported(node) {
    return (
      ts.canHaveModifiers(node) &&
      (ts.getModifiers(node) ?? []).some((m) => m.kind === ts.SyntaxKind.ExportKeyword)
    );
  }
  function visit(node) {
    if (
      (ts.isInterfaceDeclaration(node) ||
        ts.isTypeAliasDeclaration(node) ||
        ts.isClassDeclaration(node)) &&
      node.name
    ) {
      const name = node.name.text;
      // Only exported declarations count toward the public contract; a name
      // that survives solely inside `declare global` is not importable.
      if (isExported(node)) names.add(name);
      let hasDocs = false;
      function inspect(child) {
        hasDocs ||= ts.getJSDocCommentsAndTags(child).length > 0;
        ts.forEachChild(child, inspect);
      }
      inspect(node);
      if (hasDocs) documented.add(name);
    }
    ts.forEachChild(node, visit);
  }
  visit(source);
  return { names, documented };
}

const contract = JSON.parse(readFileSync(resolve(packageDir, "typegen/public-api.json"), "utf8"));
const generatedPath = resolve(packageDir, "src/generated/logic.ts");
const generatedSource = readFileSync(generatedPath, "utf8");
if (generatedSource.includes("/** *")) {
  console.error("Generated declarations contain malformed single-line JSDoc starting with '/** *'.");
  process.exit(1);
}
const generated = declarations(generatedPath);
const missing = contract.types.filter((name) => !generated.names.has(name)).sort();

if (missing.length > 0) {
  console.error(`Generated declarations are missing exported public types: ${missing.join(", ")}`);
  process.exit(1);
}

const missingDocs = contract.documented.filter((name) => !generated.documented.has(name)).sort();
if (missingDocs.length > 0) {
  console.error(`Generated declarations lost critical documentation: ${missingDocs.join(", ")}`);
  process.exit(1);
}

const profile = readFileSync(resolve(packageDir, "src/generated/logic-web.d.ts"), "utf8");
const runtime = readFileSync(
  resolve(packageDir, "../../crates/lingxia-lxapp/src/appservice/js_runtime.rs"),
  "utf8",
);
const webStandards = [
  ["fetch", "http"],
  ["Request", "http"],
  ["Response", "http"],
  ["Headers", "http"],
  ["URL", "url"],
  ["URLSearchParams", "url"],
  ["TextEncoder", "encoding"],
  ["TextDecoder", "encoding"],
  ["AbortController", "abort"],
  ["AbortSignal", "abort"],
  ["Event", "event"],
  ["EventTarget", "event"],
  ["ReadableStream", "stream"],
  ["WritableStream", "stream"],
  ["CompressionStream", "compression"],
  ["DecompressionStream", "compression"],
  ["setTimeout", "timer"],
  ["console", "console"],
];
const missingStandards = webStandards.filter(
  ([name, module]) =>
    !new RegExp(`\\b(?:interface|class|function|var|const|type)\\s+${name}\\b`).test(profile) ||
    !runtime.includes(`"${module}"`),
);
if (missingStandards.length > 0) {
  console.error(
    `Web-standard types/runtime modules are out of sync: ${missingStandards
      .map(([name, module]) => `${name} (${module})`)
      .join(", ")}`,
  );
  process.exit(1);
}

const tsc = process.platform === "win32" ? "tsc.cmd" : "tsc";
const result = spawnSync(tsc, ["-p", "tests/tsconfig.json"], {
  cwd: packageDir,
  stdio: "inherit",
  // Node >= 18.20/20.12 refuses to spawn .cmd files without a shell (CVE-2024-27980).
  shell: process.platform === "win32",
});
if (result.error) throw result.error;
if (result.status !== 0) process.exit(result.status ?? 1);

console.log(
  `Generated declarations cover all ${contract.types.length} legacy public type names and critical documentation.`,
);
console.log(`Rong Logic Web profile and runtime modules cover ${webStandards.length} standard APIs.`);
