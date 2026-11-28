// Every Logic API path the docs name must exist in `api.json`: `lx.*` in prose and JS/TS code,
// `page.*` in inline code of the skill's authoring docs, and names imported from `@lingxia/types`.
// A miss names the nearest members. A doc that names more than
// MAX_PATHS distinct members is a second API reference in the making, so it fails too.
// Reads only the checked-in catalogue, so it runs without an install.
import { readFileSync, readdirSync } from "node:fs";
import { dirname, join, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const packageDir = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const repoDir = resolve(packageDir, "../..");
const DOC_ROOTS = ["docs/skill", "docs/view"];
const MAX_PATHS = 25;
// `page` means the Logic page only in the skill's authoring docs; tests and View docs use the name for other objects.
const PAGE_DOCS = /^docs\/skill\/(?!.*testing\.md$)/;
// Members the app fills in (its generated native client): any path below them is the app's own.
const APP_DEFINED = ["page.native"];
// Stand-ins a doc writes for "some member": `lx.X(...)`, `lx.foo()`.
const PLACEHOLDERS = new Set(["X", "foo", "bar", "baz"]);
const CODE_LANGUAGES = new Set(["", "ts", "tsx", "js", "jsx", "mjs", "javascript", "typescript"]);
const IMPORT = /import\s+(?:type\s+)?\{([^}]*)\}\s+from\s+['"]@lingxia\/types(?:\/(?:page|error))?['"]/g;

const catalogue = JSON.parse(readFileSync(resolve(packageDir, "api.json"), "utf8"));
const members = new Map(catalogue.members.map(member => [member.path, member]));
const types = new Map(catalogue.types.map(type => [type.name, type]));
const EXPORTS = new Set(Object.values(catalogue.exports).flat());

function distance(a, b) {
  const row = Array.from({ length: b.length + 1 }, (_, i) => i);
  for (let i = 1; i <= a.length; i++) {
    let previous = row[0];
    row[0] = i;
    for (let j = 1; j <= b.length; j++) {
      const current = row[j];
      row[j] = Math.min(row[j] + 1, row[j - 1] + 1, previous + (a[i - 1].toLowerCase() === b[j - 1].toLowerCase() ? 0 : 1));
      previous = current;
    }
  }
  return row[b.length];
}

function nearestType(name, count = 3) {
  return [...types.keys(), ...members.keys()].filter(candidate => !candidate.includes("."))
    .map(candidate => ({ candidate, score: distance(name, candidate) }))
    .sort((a, b) => a.score - b.score || (a.candidate < b.candidate ? -1 : 1))
    .slice(0, count)
    .map(({ candidate }) => candidate);
}

// The alias table first, then edit distance on the last segment; a name inside the other
// (`getDeviceInfo`, `info`) counts as close, and a different parent as further away.
// `lingxia api` ranks misses the same way.
export function nearest(path, count = 3) {
  const segments = path.split(".");
  const last = segments.pop().toLowerCase();
  const parent = segments.join(".");
  const aliased = Object.entries(catalogue.aliases).find(([name]) => name.toLowerCase() === last)?.[1] ?? [];
  const ranked = [...members.keys()]
    .filter(candidate => !aliased.includes(candidate))
    .map(candidate => {
      const name = candidate.split(".").pop().toLowerCase();
      const contained = name.length >= 3 && (last.includes(name) || name.includes(last));
      const sameParent = !parent || candidate.slice(0, candidate.length - name.length - 1) === parent;
      return { candidate, score: distance(last, name) - (contained ? 3 : 0) + (sameParent ? 0 : 3) };
    })
    .sort((a, b) => a.score - b.score || (a.candidate < b.candidate ? -1 : 1))
    .map(({ candidate }) => candidate);
  return [...aliased, ...ranked].slice(0, count);
}

// A path resolves when it is a member, or a member followed by fields of the types it reaches
// (`lx.app.info.role`).
export function resolves(path) {
  if (APP_DEFINED.some(prefix => path.startsWith(`${prefix}.`) && members.has(prefix))) return true;
  const segments = path.split(".");
  for (let end = segments.length; end > 1; end--) {
    const member = members.get(segments.slice(0, end).join("."));
    if (!member) continue;
    if (end === segments.length) return true;
    if (member.kind === "namespace") return false;
    let reachable = member.types.map(name => types.get(name)).filter(Boolean);
    for (const segment of segments.slice(end)) {
      const fields = reachable.flatMap(type => type.fields.filter(field => field.name === segment));
      if (fields.length === 0) return false;
      reachable = [...types.values()].filter(type => fields.some(field => new RegExp(`\\b${type.name}\\b`).test(field.type)));
    }
    return true;
  }
  return false;
}

function docs(dir) {
  return readdirSync(dir, { withFileTypes: true }).flatMap(entry => {
    const full = join(dir, entry.name);
    if (entry.isDirectory()) return docs(full);
    return entry.name.endsWith(".md") ? [full] : [];
  });
}

const LX_PATH = /(?<![\w$.])lx(?:\.[A-Za-z_$][\w$]*)+/g;
const PAGE_PATH = /(?<![\w$.])page(?:\.[A-Za-z_$][\w$]*)+/g;

export function referencesIn(text, { pages = true } = {}) {
  const found = [];
  const lines = text.split("\n");
  let fence = null;
  lines.forEach((line, index) => {
    const marker = line.match(/^\s*(```|~~~)\s*([\w-]*)/);
    if (marker) {
      fence = fence === null ? marker[2].toLowerCase() : null;
      return;
    }
    const at = index + 1;
    if (fence !== null && !CODE_LANGUAGES.has(fence)) return;
    for (const [path] of line.matchAll(LX_PATH)) {
      if (!path.split(".").some(segment => PLACEHOLDERS.has(segment))) found.push({ path, line: at });
    }
    if (fence !== null || !pages) return;
    for (const [, code] of line.matchAll(/`([^`]+)`/g)) {
      for (const [path] of code.matchAll(PAGE_PATH)) found.push({ path, line: at });
    }
  });
  // Imports may span lines.
  for (const match of text.matchAll(IMPORT)) {
    const at = text.slice(0, match.index).split("\n").length;
    for (const name of match[1].split(",").map(part => part.trim().replace(/^type\s+/, "").split(/\s+as\s+/)[0]).filter(Boolean)) {
      found.push({ path: name, line: at, imported: true });
    }
  }
  return found;
}

function main() {
  const problems = [];
  let checked = 0;
  const roots = process.argv.slice(2).length ? process.argv.slice(2) : DOC_ROOTS;
  for (const root of roots) {
    for (const file of docs(resolve(repoDir, root))) {
      const name = relative(repoDir, file).split("\\").join("/");
      const references = referencesIn(readFileSync(file, "utf8"), { pages: PAGE_DOCS.test(name) });
      checked += references.length;
      for (const { path, line, imported } of references) {
        if (imported) {
          if (!members.has(path) && !types.has(path) && !EXPORTS.has(path)) problems.push(`${name}:${line}: \`@lingxia/types\` exports no \`${path}\`. Nearest: ${nearestType(path).join(", ")}`);
        } else if (!resolves(path)) {
          problems.push(`${name}:${line}: no member \`${path}\`. Nearest: ${nearest(path).join(", ")}`);
        }
      }
      const distinct = new Set(references.map(r => r.path).filter(path => members.has(path)));
      if (distinct.size > MAX_PATHS) {
        problems.push(`${name}: names ${distinct.size} API members (at most ${MAX_PATHS}); point at \`lingxia api\` instead of listing them`);
      }
    }
  }
  if (problems.length) {
    console.error(`Docs name Logic API members that do not exist (catalogue @lingxia/types ${catalogue.version}):\n  ${problems.join("\n  ")}`);
    process.exit(1);
  }
  console.log(`Docs: ${checked} Logic API references resolve.`);
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) main();
