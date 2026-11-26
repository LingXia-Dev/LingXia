#!/usr/bin/env node
// Which gallery pages a set of changed files can reach, through the static imports of each page.
//
//   node scripts/reach.mjs <git-rev>   the pages that the changes since <git-rev> (working tree included) reach
//   node scripts/reach.mjs <a>..<b>    the pages that the commits after <a> up to <b> reach
//
// It fails closed: a changed file it cannot place in any page's imports (a theme, a token, an icon,
// a catalogue, an asset, anything outside Liuli) reaches every page, and so does a page it cannot read.
// New files count only inside Liuli: elsewhere one reaches a page only through a changed file that imports it.
// Tests, Markdown and the gallery's own probes reach none.
import { execFileSync } from 'node:child_process';
import { existsSync, readFileSync } from 'node:fs';
import { dirname, join, relative, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const liuli = resolve(dirname(fileURLToPath(import.meta.url)), '..');
// The packages the gallery draws, by their published names.
const manifests = JSON.parse(readFileSync(join(liuli, 'gallery', 'packages.json'), 'utf8')).map(dir => [join(liuli, dir), JSON.parse(readFileSync(join(liuli, dir, 'package.json'), 'utf8'))]);
const packages = Object.fromEntries(manifests.map(([dir, manifest]) => [manifest.name, dir]));
const exportsOf = Object.fromEntries(manifests.map(([, manifest]) => [manifest.name, manifest.exports ?? {}]));

// An export by its subpath, an exact key first, then a `*` pattern such as `./Icon/icons/*`.
function exportTarget(map, sub) {
  if (typeof map[sub] === 'string') return map[sub];
  for (const [key, target] of Object.entries(map)) {
    const star = key.indexOf('*');
    if (star < 0 || typeof target !== 'string') continue;
    const [head, tail] = [key.slice(0, star), key.slice(star + 1)];
    if (sub.length >= head.length + tail.length && sub.startsWith(head) && sub.endsWith(tail)) return target.replace('*', sub.slice(head.length, sub.length - tail.length));
  }
  return undefined;
}

function resolveImport(from, spec) {
  if (spec.startsWith('.')) {
    const base = resolve(dirname(from), spec);
    for (const candidate of [base, `${base}.tsx`, `${base}.ts`, `${base}.mjs`, join(base, 'index.ts'), join(base, 'index.tsx')]) if (existsSync(candidate) && !candidate.endsWith('/') && candidate.match(/\.(tsx?|mjs)$/)) return candidate;
    return null;
  }
  const name = Object.keys(packages).find(pkg => spec === pkg || spec.startsWith(`${pkg}/`));
  if (!name) return undefined; // outside Liuli: the View, the logic, node
  const target = exportTarget(exportsOf[name], `.${spec.slice(name.length)}`);
  return typeof target === 'string' ? join(packages[name], target) : null;
}

// Every file a page's entry files import, transitively; null when an import of Liuli's cannot be resolved.
function closure(entries) {
  const seen = new Set();
  const queue = [...entries];
  while (queue.length) {
    const file = queue.pop();
    if (seen.has(file)) continue;
    seen.add(file);
    const source = readFileSync(file, 'utf8');
    for (const [, spec] of source.matchAll(/(?:from|import)\s*\(?\s*['"]([^'"]+)['"]/g)) {
      const target = resolveImport(file, spec);
      if (target === null) return null;
      if (target) queue.push(target);
    }
  }
  return seen;
}

export function pagesReaching(changed, gallery = join(liuli, 'gallery')) {
  const pages = JSON.parse(readFileSync(join(gallery, 'lxapp.json'), 'utf8')).pages;
  const every = pages.map(page => page.name);
  const reach = new Map();
  for (const page of pages) {
    const dir = join(gallery, dirname(page.path));
    const entries = ['index.tsx', 'index.ts'].map(name => join(dir, name)).filter(existsSync);
    const files = entries.length ? closure(entries) : null;
    if (!files) return { pages: every, why: `${page.name}: its imports cannot be read` };
    reach.set(page.name, files);
  }
  const hit = new Set();
  for (const file of changed.map(path => resolve(path))) {
    if (/\.(test\.(mjs|ts|tsx)|md)$/.test(file) || file.includes('/zz-tests/') || file.includes('/test-results/') || file.includes('/tests/fixtures/') || file.includes('/src/testing/')) continue;
    // Derived or tooling: a catalogue, the generators and their gallery content, which is itself in the diff;
    // the word catalogues are drawn through their generated words.ts; a deleted Liuli file through its importers.
    if (/\/liuli\/(core|charts|jinshu)\/catalog\.json$|\/liuli\/(\w+\/)?scripts\/|\/i18n\/liuli\/[\w-]+\.json$/.test(file) || (!existsSync(file) && file.startsWith(`${liuli}/`))) continue;
    // A doc file is drawn through its own examples, and a page's json and logic through its page.
    const drawn = file.replace(/\.doc\.ts$/, '.examples.tsx');
    const owner = pages.find(page => dirname(file) === join(gallery, dirname(page.path)));
    const reached = owner ? [owner.name] : every.filter(name => reach.get(name).has(drawn));
    if (!reached.length) return { pages: every, why: `${relative(liuli, file)} reaches no page by its imports` };
    for (const name of reached) hit.add(name);
  }
  return { pages: every.filter(name => hit.has(name)), why: null };
}

export function changedSince(rev) {
  const top = execFileSync('git', ['rev-parse', '--show-toplevel'], { cwd: liuli, encoding: 'utf8' }).trim();
  const listed = args => execFileSync('git', args, { cwd: top, encoding: 'utf8' }).split('\n').filter(Boolean);
  // `a..b` is two commits; a single rev is that commit against the working tree, new files included.
  const fresh = rev.includes('..') ? [] : listed(['ls-files', '--others', '--exclude-standard', '--', relative(top, liuli)]);
  return [...listed(['diff', '--name-only', rev, '--']), ...fresh].map(path => join(top, path));
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const rev = process.argv[2];
  if (!rev) throw Error('usage: reach.mjs <git-rev>');
  const { pages, why } = pagesReaching(changedSince(rev));
  console.log(why ? `every page (${why})` : pages.join(',') || '(none)');
}
