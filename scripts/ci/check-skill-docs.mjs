#!/usr/bin/env node
// Keep the agent skill self-contained and its links working.
//
// `lingxia skill install` copies docs/skill alone, so a skill doc that points
// outside that tree hands the agent a dead reference. Every relative link must
// resolve to a file inside docs/skill, every #anchor to a heading there, and no
// doc may name a repository path (docs/internal, tools/, crates/, ...) as
// reading material. Public https:// links are allowed.
//
// With `--package <dir>` (repeatable) the skill ships inside those npm packages,
// and every name it puts in backticks must exist there: colour classes as theme
// roles, type steps as token names, parts and types in the catalogues, props as
// catalogue props or the naming conventions, imports and package paths as files.
// It cannot catch a wrong value of a prop some part types as a free string, nor an
// invented prop shaped like a convention (`isWhatever`).
import fs from 'node:fs';
import path from 'node:path';
import process from 'node:process';
import { fileURLToPath } from 'node:url';

const args = process.argv.slice(2);
const packageDirs = [];
for (let i = args.indexOf('--package'); i !== -1; i = args.indexOf('--package')) {
  packageDirs.push(path.resolve(args[i + 1]));
  args.splice(i, 2);
}
const root = path.resolve(args[0] ?? 'docs/skill');
const REPO_PATH = /(^|[\s`'"(\[])(?:\.\.\/)*(docs|tools|crates|packages|examples|scripts|website|lingxia-sdk|\.github)\/[\w.-]/;

function walk(dir) {
  return fs.readdirSync(dir, { withFileTypes: true }).flatMap((entry) => {
    const full = path.join(dir, entry.name);
    if (entry.isDirectory()) return walk(full);
    return entry.name.endsWith('.md') ? [full] : [];
  });
}

// GitHub heading slugs: strip code ticks and link targets, drop punctuation,
// spaces to hyphens, number repeats.
function slug(text) {
  return text
    .replace(/`/g, '')
    .replace(/\[([^\]]*)\]\([^)]*\)/g, '$1')
    .trim()
    .toLowerCase()
    .replace(/[^\p{L}\p{N}_\- ]/gu, '')
    .replace(/ /g, '-');
}

const anchorCache = new Map();
function anchors(file) {
  if (anchorCache.has(file)) return anchorCache.get(file);
  const seen = new Map();
  const out = new Set();
  let fence = false;
  for (const line of fs.readFileSync(file, 'utf8').split('\n')) {
    if (/^\s*(```|~~~)/.test(line)) { fence = !fence; continue; }
    if (fence) continue;
    const m = /^(#{1,6})\s+(.*?)\s*#*\s*$/.exec(line);
    if (!m) continue;
    const s = slug(m[2]);
    const n = seen.get(s) ?? 0;
    seen.set(s, n + 1);
    out.add(n === 0 ? s : `${s}-${n}`);
  }
  anchorCache.set(file, out);
  return out;
}

const names = packageDirs.length ? packageNames(packageDirs) : null;

function packageNames(dirs) {
  const repo = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
  const schema = JSON.parse(fs.readFileSync(path.join(repo, 'contracts/view/schema.json'), 'utf8'));
  // A role's Tailwind colour drops a leading `text-`: role `text-muted` is class `text-muted`.
  const colours = new Set(schema.roles.map((role) => role.name.replace(/^text-(?=.)/, '')));
  const typeSteps = new Set(['xs', 'sm', 'base', 'lg', 'xl', ...[2, 3, 4, 5, 6, 7, 8, 9].map((n) => `${n}xl`)]);
  const parts = new Set();
  const props = new Map();
  const fields = new Set();
  const packages = new Map();
  for (const dir of dirs) {
    const manifest = JSON.parse(fs.readFileSync(path.join(dir, 'package.json'), 'utf8'));
    packages.set(manifest.name, { dir, exports: manifest.exports ?? {} });
    const { catalog, names: tokenNames } = manifest.lingxia ?? {};
    if (tokenNames) {
      for (const step of Object.keys(JSON.parse(fs.readFileSync(path.join(dir, tokenNames), 'utf8')).type ?? {})) typeSteps.add(step);
    }
    if (!catalog) continue;
    for (const entry of JSON.parse(fs.readFileSync(path.join(dir, catalog), 'utf8')).components) {
      for (const key of Object.keys(entry)) fields.add(key);
      parts.add(entry.name);
      for (const name of entry.exports ?? []) parts.add(name);
      for (const prop of [...(entry.props ?? []), ...Object.values(entry.otherProps ?? {}).flat()]) {
        if (!props.has(prop.name)) props.set(prop.name, []);
        props.get(prop.name).push(prop.type);
        for (const word of String(prop.type).match(/\b[A-Z][A-Za-z]+\b/g) ?? []) parts.add(word);
      }
    }
  }
  return { colours, typeSteps, parts, props, fields, packages };
}

const COLOUR = /^(text|bg|border|ring|fill|stroke|divide|outline)-([a-z][a-z0-9-]*)(\/\d+)?$/;
const STEP = /^(gap|space-[xy]|[pm][xytrblse]?|size|[wh])-(\d+(\.5)?|px)$/;
const CONVENTION = /^(is|has|on)[A-Z]\w*$|^data-[a-z-]+$/;

function packagePath(spec) {
  for (const [name, pkg] of names.packages) {
    if (spec !== name && !spec.startsWith(`${name}/`)) continue;
    return { pkg, rest: spec.slice(name.length + 1) };
  }
  return null;
}

// Problems with one backticked name, or null when it is fine or not a name this mode knows.
function checkName(code) {
  const [, prefix, colour] = COLOUR.exec(code) ?? [];
  if (prefix) {
    if (prefix === 'text' && names.typeSteps.has(colour)) return null;
    return names.colours.has(colour) ? null : `\`${code}\` names no theme role or type step`;
  }
  if (STEP.test(code)) return null;
  const imported = /^import\s.*\sfrom\s+['"]([^'"]+)['"];?$/.exec(code)?.[1];
  if (imported) {
    const found = packagePath(imported);
    if (!found) return null;
    const key = found.rest ? `./${found.rest}` : '.';
    const exported = Object.keys(found.pkg.exports).some((pattern) =>
      pattern === key || (pattern.endsWith('/*') && key.startsWith(pattern.slice(0, -1))));
    return exported ? null : `\`${imported}\` is not an export of its package`;
  }
  const moduleFile = /^node_modules\/(@[^/]+\/[^/]+|[^@/][^/]*)\/(.+)$/.exec(code);
  if (moduleFile) {
    const pkg = names.packages.get(moduleFile[1]);
    if (!pkg) return null;
    const file = moduleFile[2].replace(/<[^>]+>[^/]*$/, '');
    return fs.existsSync(path.join(pkg.dir, file)) ? null : `\`${code}\` is not in ${moduleFile[1]}`;
  }
  const attribute = /^([a-z][A-Za-z]*)="([^"]*)"$/.exec(code);
  if (attribute) {
    const types = names.props.get(attribute[1]);
    if (!types) return `\`${attribute[1]}\` is no catalogue prop`;
    const literal = types.some((type) => !/"/.test(type) || type.includes(`"${attribute[2]}"`));
    return literal ? null : `\`${code}\`: "${attribute[2]}" is not one of the prop's values`;
  }
  if (/^[A-Z][A-Za-z]+$/.test(code)) {
    return names.parts.has(code) ? null : `\`${code}\` is no part, export or prop type in the catalogues`;
  }
  if (/^[a-z][A-Za-z]*$|^data-[a-z-]+$/.test(code)) {
    if (names.props.has(code) || names.fields.has(code) || CONVENTION.test(code)) return null;
    return `\`${code}\` is no catalogue prop, catalogue field or naming convention`;
  }
  return null;
}

const problems = [];
const files = walk(root);
for (const file of files) {
  const rel = path.relative(process.cwd(), file);
  let fence = false;
  fs.readFileSync(file, 'utf8').split('\n').forEach((line, i) => {
    const where = `${rel}:${i + 1}`;
    if (/^\s*(```|~~~)/.test(line)) { fence = !fence; return; }
    if (REPO_PATH.test(line)) problems.push(`${where}: names a repository path outside the skill`);
    if (fence) return;
    if (names) {
      // A name shown as the wrong form ("never `testId`") is not checked.
      for (const m of line.matchAll(/`([^`]+)`/g)) {
        if (/\b(never|not)\s+$/i.test(line.slice(0, m.index))) continue;
        const problem = checkName(m[1]);
        if (problem) problems.push(`${where}: ${problem}`);
      }
    }
    const prose = line.replace(/`[^`]*`/g, (code) => (code.includes('](') ? code : '``'));
    for (const [, target] of prose.matchAll(/(?<!!)\[[^\]]*\]\(([^)\s]+)(?:\s+"[^"]*")?\)/g)) {
      if (/^[a-z][a-z0-9+.-]*:/i.test(target)) {
        if (!/^(https|mailto):/i.test(target)) problems.push(`${where}: non-https link ${target}`);
        continue;
      }
      const [p, frag] = target.split('#');
      const dest = p ? path.resolve(path.dirname(file), p) : file;
      if (dest !== root && !dest.startsWith(root + path.sep)) {
        problems.push(`${where}: link leaves the skill: ${target}`);
        continue;
      }
      if (!fs.existsSync(dest)) { problems.push(`${where}: missing file ${target}`); continue; }
      if (frag && dest.endsWith('.md') && !anchors(dest).has(frag)) {
        problems.push(`${where}: missing anchor ${target}`);
      }
    }
  });
}

for (const p of problems) console.error(p);
console.log(`${files.length} skill doc(s), ${problems.length} problem(s)`);
process.exit(problems.length ? 1 : 0);
