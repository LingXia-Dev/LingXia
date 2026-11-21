// Every code example in the declarations' doc comments (`@example` and ```ts fences) type-checks
// against the package as an lxapp's Logic sees it. Names an example leaves undefined are
// placeholders (`presignedUrl`, `render`) and read as `any`; every other error fails.
import { readFileSync, readdirSync } from "node:fs";
import { dirname, join, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import ts from "typescript";

const packageDir = resolve(dirname(fileURLToPath(import.meta.url)), "..");
// The test API (`automation/`) is checked with `@lingxia/test`.
const SOURCES = ["src/index.ts", "src/page.ts", "src/error.ts", "src/mocks.ts", "src/generated/logic.ts"];
const PLACEHOLDER = new Set([2304, 2552, 2582]);
const PROVIDED = { "@lingxia/logic": ["definePage", "type", "isLxError", "lx"] };

export function examplesIn(file) {
  const text = readFileSync(file, "utf8");
  const found = [];
  for (const comment of text.matchAll(/\/\*\*[^]*?\*\//g)) {
    const body = comment[0].split("\n").map(line => line.replace(/^\s*\/?\*+\/?\s?/, "")).join("\n");
    const line = text.slice(0, comment.index).split("\n").length;
    for (const fence of body.matchAll(/```(?:ts|typescript|js)\n([^]*?)```/g)) {
      const offset = body.slice(0, fence.index).split("\n").length;
      found.push({ file, line: line + offset, code: fence[1].replace(/\\\//g, "/") });
    }
    // An `@example` written without a fence runs to the next tag.
    for (const bare of body.matchAll(/^@example[ \t]*\n(?!\s*```)([^]*?)(?=^@|$(?![^]))/gm)) {
      const offset = body.slice(0, bare.index).split("\n").length;
      found.push({ file, line: line + offset, code: bare[1] });
    }
  }
  return found;
}

function main() {
  const examples = SOURCES.flatMap(source => examplesIn(resolve(packageDir, source)));
  const virtual = new Map();
  examples.forEach((example, index) => {
    const imports = Object.entries(PROVIDED)
      .map(([module, names]) => [module, names.filter(name => new RegExp(`\\b${name}\\b`).test(example.code) && !new RegExp(`import[^;]*\\b${name}\\b`).test(example.code))])
      .filter(([, names]) => names.length)
      .map(([module, names]) => `import { ${names.join(", ")} } from '${module}';`);
    const head = imports.join(" ");
    // A block that shows alternatives (`export default definePage(…)` twice) checks each one.
    let alternative = 0;
    example.code = example.code.replace(/^export default /gm, () => `export const example${alternative++} = `);
    virtual.set(resolve(packageDir, `.examples/example-${index}.ts`), { example, head, text: `${head}\n${example.code}\nexport {};\n` });
  });
  const options = {
    target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.ESNext, moduleResolution: ts.ModuleResolutionKind.Bundler,
    strict: true, noEmit: true, skipLibCheck: true, types: [], lib: ["lib.es2022.d.ts"],
    baseUrl: packageDir,
    paths: { "@lingxia/types": ["src/index.ts"], "@lingxia/types/*": ["src/*"], "@lingxia/logic": ["../lingxia-logic/index.d.ts"] },
  };
  const host = ts.createCompilerHost(options);
  const readFile = host.readFile.bind(host);
  const fileExists = host.fileExists.bind(host);
  const getSourceFile = host.getSourceFile.bind(host);
  host.readFile = name => virtual.get(resolve(name))?.text ?? readFile(name);
  host.fileExists = name => virtual.has(resolve(name)) || fileExists(name);
  host.getSourceFile = (name, version, ...rest) => {
    const entry = virtual.get(resolve(name));
    return entry ? ts.createSourceFile(name, entry.text, version, true) : getSourceFile(name, version, ...rest);
  };
  const roots = [resolve(packageDir, "src/index.ts"), resolve(packageDir, "src/logic-globals.d.ts"), ...virtual.keys()];
  const program = ts.createProgram(roots, options, host);
  const problems = [];
  for (const [name, { example }] of virtual) {
    for (const diagnostic of ts.getPreEmitDiagnostics(program, program.getSourceFile(name))) {
      if (PLACEHOLDER.has(diagnostic.code)) continue;
      const { line } = diagnostic.file.getLineAndCharacterOfPosition(diagnostic.start);
      const message = ts.flattenDiagnosticMessageText(diagnostic.messageText, "\n    ");
      problems.push(`${relative(packageDir, example.file)}:${example.line + line - 1}: TS${diagnostic.code} ${message}`);
    }
  }
  if (problems.length) {
    console.error(`Doc examples that do not type-check:\n  ${problems.join("\n  ")}`);
    process.exit(1);
  }
  console.log(`Doc examples: ${examples.length} type-check.`);
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) main();
