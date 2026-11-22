// A Rust struct the TS declarations skip (`#[ts_skip]`) has a hand-written TS twin; their field
// names must agree, or an option is silently ignored or a result field goes undeclared.
// Twins are paired through the registrations: `fn location(ts_params = "options?: GetLocationOptions") = get_location;`
// with `async fn get_location(ctx: JSContext, options: Optional<JSLocationOptions>)`.
import { readdirSync, readFileSync, statSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import ts from "typescript";

const packageDir = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const crateDir = resolve(packageDir, "../../crates/lingxia-logic/src");

const files = (function walk(dir) {
  return readdirSync(dir).flatMap(name => {
    const path = join(dir, name);
    return statSync(path).isDirectory() ? walk(path) : path.endsWith(".rs") ? [path] : [];
  });
})(crateDir);

const camel = name => name.replace(/_([a-z])/g, (_, c) => c.toUpperCase());
const structs = new Map();
const functions = new Map();
const registrations = [];
for (const file of files) {
  const source = readFileSync(file, "utf8");
  for (const match of source.matchAll(/#\[ts_skip\]\s*(?:#\[[^\]]*\]\s*)*(?:pub(?:\([^)]*\))?\s+)?struct (\w+)\s*\{([\s\S]*?)\n\}/g)) {
    const [, name, body] = match;
    const fields = [];
    for (const field of body.matchAll(/((?:\s*#\[[^\]]*\]\s*|\s*\/\/[^\n]*\n)*)\s*(?:pub(?:\([^)]*\))?\s+)?(\w+)\s*:/g)) {
      const [, attributes, ident] = field;
      if (/#\[js_skip\]/.test(attributes)) continue;
      fields.push(attributes.match(/#\[js_name\s*=\s*"([^"]+)"\]/)?.[1] ?? camel(ident));
    }
    structs.set(name, { file, fields });
  }
  for (const match of source.matchAll(/fn (\w+)(?:<[^>]*>)?\(([\s\S]*?)\)\s*(?:->\s*([^{;]+))?\{/g)) {
    const [, name, params, returns] = match;
    functions.set(name, { params: params.split(",").map(param => param.split(":").slice(1).join(":").trim()).filter(Boolean), returns: returns?.trim() ?? "" });
  }
  for (const match of source.matchAll(/fn (\w+)\(\s*((?:ts_\w+\s*=\s*"[^"]*"\s*,?\s*)+)\)\s*=\s*(\w+);/g)) {
    const [, jsName, attributes, target] = match;
    registrations.push({
      jsName, target, file,
      params: attributes.match(/ts_params\s*=\s*"([^"]*)"/)?.[1],
      returns: attributes.match(/ts_return\s*=\s*"([^"]*)"/)?.[1],
    });
  }
}

// TS types are resolved in a module that imports every generated name.
const logicPath = resolve(packageDir, "src/generated/logic.ts");
const base = ts.createProgram([logicPath], { noEmit: true, skipLibCheck: true, target: ts.ScriptTarget.ES2022, types: [] });
const exports = base.getTypeChecker().getExportsOfModule(base.getTypeChecker().getSymbolAtLocation(base.getSourceFile(logicPath)));
// TypeScript hands the host forward-slash paths, also on Windows.
const probePath = resolve(packageDir, "src/__twins__.ts").replaceAll("\\", "/");
const probes = [];
const typeFor = text => { probes.push(text); return `__T${probes.length - 1}`; };

const pairs = [];
const unwrapRust = type => {
  let current = type.replace(/\s+/g, "");
  for (;;) {
    const inner = current.match(/^(?:Optional|Option|JSResult|Result|Vec|Box|rong::function::Optional|function::Optional)<(.+)>$/)?.[1];
    if (!inner) return current.split("<")[0].split("::").pop();
    current = inner.split(",")[0];
  }
};
const unwrapTs = type => {
  let current = type.trim();
  for (;;) {
    const inner = current.match(/^(?:Promise|ReadonlyArray|Array)<([\s\S]+)>$/)?.[1];
    if (!inner) return current.replace(/\[\]$/, "");
    current = inner.trim();
  }
};
const tsParams = text => {
  const params = [];
  let depth = 0, start = 0;
  for (let i = 0; i <= text.length; i++) {
    const c = text[i];
    if (c === "<" || c === "{" || c === "(" || c === "[") depth++;
    else if (c === ">" || c === "}" || c === ")" || c === "]") depth--;
    else if ((c === "," && depth === 0) || i === text.length) { params.push(text.slice(start, i).trim()); start = i + 1; }
  }
  return params.filter(Boolean).map(param => param.replace(/^\.\.\./, "").replace(/^\w+\??\s*:\s*/, ""));
};
for (const registration of registrations) {
  const target = functions.get(registration.target);
  if (!target) continue;
  const rustParams = target.params.filter(param => !/JSContext|^This<|^&?JSContext/.test(param));
  const typeScript = registration.params ? tsParams(registration.params) : [];
  rustParams.forEach((param, index) => {
    const struct = structs.get(unwrapRust(param));
    if (struct && typeScript[index]) pairs.push({ struct: unwrapRust(param), ...struct, side: "options", where: `${registration.jsName}(${typeScript[index]})`, probe: typeFor(unwrapTs(typeScript[index])) });
  });
  const struct = structs.get(unwrapRust(target.returns));
  if (struct && registration.returns) pairs.push({ struct: unwrapRust(target.returns), ...struct, side: "result", where: `${registration.jsName}(): ${registration.returns}`, probe: typeFor(unwrapTs(registration.returns)) });
}

const probeSource = `import type { ${exports.map(symbol => symbol.name).join(", ")} } from "./generated/logic.js";\n` +
  probes.map((text, index) => `export type __T${index} = ${text};`).join("\n");
const host = ts.createCompilerHost({});
const read = host.readFile.bind(host);
host.readFile = path => path === probePath ? probeSource : read(path);
host.fileExists = (exists => path => path === probePath || exists(path))(host.fileExists.bind(host));
const getSource = host.getSourceFile.bind(host);
host.getSourceFile = (path, ...rest) => path === probePath ? ts.createSourceFile(path, probeSource, ts.ScriptTarget.ES2022, true) : getSource(path, ...rest);
const program = ts.createProgram([probePath], { noEmit: true, skipLibCheck: true, target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.NodeNext, moduleResolution: ts.ModuleResolutionKind.NodeNext, types: [] }, host);
const checker = program.getTypeChecker();
const module = checker.getSymbolAtLocation(program.getSourceFile(probePath));
const probeTypes = new Map(checker.getExportsOfModule(module).map(symbol => [symbol.name, checker.getDeclaredTypeOfSymbol(symbol)]));

const problems = [];
let checked = 0;
for (const pair of pairs) {
  const type = probeTypes.get(pair.probe);
  const declared = new Set(checker.getPropertiesOfType(checker.getNonNullableType(type)).map(symbol => symbol.name));
  if (!declared.size) continue;
  checked++;
  const rust = new Set(pair.fields);
  const onlyRust = [...rust].filter(name => !declared.has(name));
  const onlyTs = [...declared].filter(name => !rust.has(name));
  // An option the TS declares but Rust never reads is silently ignored; a result field Rust
  // returns but TS does not declare is invisible to the editor.
  if (pair.side === "options" && onlyTs.length) problems.push(`${pair.where}: ${pair.struct} does not read ${onlyTs.join(", ")}`);
  if (pair.side === "options" && onlyRust.length) problems.push(`${pair.where}: ${pair.struct} reads ${onlyRust.join(", ")}, which the type does not declare`);
  if (pair.side === "result" && onlyRust.length) problems.push(`${pair.where}: ${pair.struct} returns ${onlyRust.join(", ")}, which the type does not declare`);
  if (pair.side === "result" && onlyTs.length) problems.push(`${pair.where}: the type declares ${onlyTs.join(", ")}, which ${pair.struct} never returns`);
}
if (problems.length) {
  console.error(`Rust structs and their TS twins disagree:\n  ${problems.join("\n  ")}`);
  process.exit(1);
}
console.log(`Twins: ${checked} Rust structs agree with their TS types.`);
