// The Logic API catalogue for tools: `api.json`, read by `lingxia api`. One entry per member
// (`lx.*`, `App`, `definePage` and `page.*`, the error helpers) and per named type they reach.
// Writes `api.json` and `src/testing/lx-mock-shape.ts`; `--out <file>` writes the catalogue elsewhere (the build's `dist/api.json`); `--check` fails on drift.
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { catalogue } from "./catalogue.mjs";

const SCHEMA = 1;
const packageDir = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const checkedIn = resolve(packageDir, "api.json");
const mockShapeFile = resolve(packageDir, "src/testing/lx-mock-shape.ts");

const LIST_TAGS = ["platform", "permission", "supports", "see"];

function anchor(source) {
  return source ? `${source.file}:${source.line}` : null;
}

// Fenced code in an `@example`, without the fence.
function exampleCode(text) {
  const fenced = text.match(/```\w*\n([^]*?)```/);
  return (fenced ? fenced[1] : text).replace(/\s+$/, "");
}

function structuredTags(jsdoc) {
  const out = {};
  for (const { name, text } of jsdoc) {
    const value = text.replace(/\s+/g, " ").trim();
    if (name === "throws") {
      const match = value.match(/^\{([^}]+)\}\s*(.*)$/);
      (out.throws ??= []).push(match ? { code: match[1], when: match[2] } : { code: null, when: value });
    } else if (name === "default") {
      out.default = value;
    } else if (name === "deprecated") {
      out.deprecated = value || true;
    } else if (LIST_TAGS.includes(name)) {
      (out[name] ??= []).push(value);
    }
  }
  return out;
}

function examples(jsdoc) {
  return jsdoc.filter(tag => tag.name === "example").map(tag => exampleCode(tag.text));
}

function kindOf(member) {
  const name = member.path.split(".").pop();
  if (member.kind === "namespace") return "namespace";
  if (member.kind === "value") return "property";
  if (/^(App|page)\.on[A-Z]/.test(member.path) && member.path.split(".").length === 2) return "hook";
  if (/^on[A-Z]/.test(name)) return "event";
  if (/^watch([A-Z]|$)/.test(name)) return "watcher";
  return "function";
}

function field(entry) {
  const tags = structuredTags(entry.jsdoc);
  const { default: fallback, ...rest } = tags;
  return {
    name: entry.name,
    type: entry.type,
    required: !entry.optional,
    summary: entry.summary,
    description: entry.doc,
    ...(fallback !== undefined ? { default: fallback } : {}),
    ...(Object.keys(rest).length ? { tags: rest } : {}),
  };
}

export function apiCatalogue() {
  const { members, types, typeInfo, aliases, exports } = catalogue({ entries: true });
  const version = JSON.parse(readFileSync(resolve(packageDir, "package.json"), "utf8")).version;

  const typeEntries = new Map();
  for (const [name, fields] of types) {
    const info = typeInfo.get(name) ?? {};
    typeEntries.set(name, {
      name,
      summary: info.summary ?? "",
      description: info.doc ?? "",
      fields: fields.filter(f => !f.name.startsWith("__@")).map(field),
      ...(info.jsdoc?.length ? { examples: examples(info.jsdoc) } : {}),
      source: anchor(info.source ?? fields[0]?.source),
    });
  }
  for (const [name, info] of aliases) {
    if (typeEntries.has(name)) continue;
    typeEntries.set(name, { name, summary: info.summary, description: info.doc, text: info.text, fields: [], source: anchor(info.source) });
  }
  for (const entry of typeEntries.values()) if (!entry.examples?.length) delete entry.examples;

  // Named types a member reaches: what its signatures name, then what those types' fields name.
  const names = [...typeEntries.keys()].filter(name => /^\w+$/.test(name));
  const namePattern = new RegExp(`\\b(${names.sort((a, b) => b.length - a.length).join("|")})\\b`, "g");
  const named = text => [...new Set([...(text ?? "").matchAll(namePattern)].map(([name]) => name))];
  function reach(member) {
    const texts = member.signatures.map(s => s.text).concat(member.type ?? []);
    const found = new Set(named(texts.join("\n")));
    // An inline option or result object is filed under the member: `lx.ui.alert(options)`.
    for (const name of typeEntries.keys()) if (name.startsWith(`${member.path}(`)) found.add(name);
    for (const name of [...found]) {
      const entry = typeEntries.get(name);
      for (const next of named([entry.text, ...entry.fields.map(f => f.type)].join("\n"))) found.add(next);
    }
    return [...found].sort();
  }

  const out = members.map(member => {
    const tags = structuredTags(member.jsdoc);
    return {
      path: member.path,
      kind: kindOf(member),
      entry: member.entry ?? "global",
      summary: member.summary,
      description: member.doc,
      // `(url: never): never` overloads exist only to reject a call; they are not a way to call it.
      signatures: member.signatures.filter(signature => signature.returns !== "never").map(signature => ({
        text: `${member.path.split(".").pop()}${signature.written ?? signature.text}`,
        parameters: signature.parameters.map(p => ({ name: p.name, type: p.written ?? p.type, required: !p.optional })),
        returns: signature.returns,
      })),
      ...(member.type ? { type: member.type } : {}),
      types: reach(member),
      tags,
      examples: examples(member.jsdoc),
      source: anchor(member.source),
    };
  });

  const byString = (a, b) => (a < b ? -1 : a > b ? 1 : 0);
  const paths = new Set(out.map(member => member.path));
  const guessed = JSON.parse(readFileSync(resolve(packageDir, "typegen/api-aliases.json"), "utf8")).aliases;
  const stale = Object.entries(guessed).flatMap(([name, targets]) => targets.filter(t => !paths.has(t)).map(t => `${name} -> ${t}`));
  if (stale.length) throw Error(`typegen/api-aliases.json names members that do not exist:\n  ${stale.join("\n  ")}`);
  return {
    generated: "Generated from the declarations by scripts/emit-api-json.mjs; do not edit.",
    schema: SCHEMA,
    package: "@lingxia/types",
    version,
    // Names each entry point exports, for import hints.
    exports,
    // Names from other runtimes, offered on a miss.
    aliases: Object.fromEntries(Object.keys(guessed).sort(byString).map(name => [name, guessed[name]])),
    members: out.sort((a, b) => byString(a.path, b.path)),
    types: [...typeEntries.values()].sort((a, b) => byString(a.name, b.name)),
  };
}

// What `createLxMock` needs to stand in for a member it was not given: a namespace
// to descend into, a call that rejects or throws, or a property read that throws.
function mockShapeSource(catalogue) {
  const shape = catalogue.members
    .filter(member => member.entry === "global" && member.path.startsWith("lx."))
    .map(member => {
      const kind = member.kind === "namespace" ? "namespace"
        : member.kind === "property" ? "value"
        : member.signatures.length && member.signatures.every(s => /^Promise</.test(s.returns)) ? "async"
        : "sync";
      return `  ${JSON.stringify(member.path)}: ${JSON.stringify(kind)},`;
    });
  return `// Generated by scripts/emit-api-json.mjs from the Lx declarations; do not edit.
/** Each \`lx\` member's kind, for \`createLxMock\`. */
export const LX_MOCK_SHAPE: Readonly<Record<string, 'namespace' | 'async' | 'sync' | 'value'>> = {
${shape.join("\n")}
};
`;
}

function main(argv) {
  const catalogue = apiCatalogue();
  // Doc text read on a CRLF checkout carries `\r`; the output is the same on every OS.
  const lf = (_key, value) => (typeof value === "string" ? value.replace(/\r\n?/g, "\n") : value);
  const json = `${JSON.stringify(catalogue, lf, 2)}\n`;
  const shape = mockShapeSource(catalogue).replace(/\r\n?/g, "\n");
  if (argv.includes("--check")) {
    let current = null;
    try { current = readFileSync(checkedIn, "utf8"); } catch {}
    if (current !== json) {
      console.error("packages/lingxia-types/api.json is stale; run `npm run gen:api`");
      process.exit(1);
    }
    let currentShape = null;
    try { currentShape = readFileSync(mockShapeFile, "utf8"); } catch {}
    if (currentShape !== shape) {
      console.error("packages/lingxia-types/src/testing/lx-mock-shape.ts is stale; run `npm run gen:api`");
      process.exit(1);
    }
    console.log(`api.json is up to date (${JSON.parse(json).members.length} members).`);
    return;
  }
  const at = argv.indexOf("--out");
  const target = at >= 0 ? resolve(process.cwd(), argv[at + 1]) : checkedIn;
  mkdirSync(dirname(target), { recursive: true });
  writeFileSync(target, json);
  console.log(`wrote ${target}`);
  if (at < 0) writeFileSync(mockShapeFile, shape);
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) main(process.argv.slice(2));
