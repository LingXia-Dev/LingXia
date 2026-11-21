import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync, readdirSync, statSync } from 'node:fs';
import { basename, join } from 'node:path';
import { fileURLToPath } from 'node:url';

// Device probes run compile-only in CI, so a renamed testId would otherwise surface only on a phone.
const repo = fileURLToPath(new URL('../../../', import.meta.url));
const gpu = join(repo, 'lingxia-sdk/android/tests/gpu');
const config = JSON.parse(readFileSync(join(gpu, 'probe-targets.json'), 'utf8'));
const walk = (dir, pattern) => readdirSync(dir, { withFileTypes: true }).flatMap(entry =>
  ['node_modules', 'dist'].includes(entry.name) ? [] :
  entry.isDirectory() ? walk(join(dir, entry.name), pattern) :
  pattern.test(entry.name) ? [join(dir, entry.name)] : []);
const testIdsIn = source => [...source.matchAll(/testId=\{?["']([A-Za-z0-9-]+)["']/g)].map(match => match[1]);

// Any literal may reach a testId through props or data; composed ids are `${testId}-${value}`.
function idSet(dirs) {
  const literals = new Set(), testIds = new Set(), prefixes = new Set();
  for (const file of dirs.flatMap(dir => walk(join(repo, dir), /\.tsx?$/))) {
    const source = readFileSync(file, 'utf8');
    for (const [, , value] of source.matchAll(/(["'])([A-Za-z0-9][A-Za-z0-9-]*)\1/g)) literals.add(value);
    for (const value of testIdsIn(source)) testIds.add(value);
    for (const [, value] of source.matchAll(/`([A-Za-z0-9][A-Za-z0-9-]*-)\$\{/g)) prefixes.add(value);
  }
  return id => literals.has(id) || [...prefixes].some(prefix => id.startsWith(prefix) && id.length > prefix.length) ||
    [...testIds].some(base => id.startsWith(`${base}-`) && literals.has(id.slice(base.length + 1)));
}

function sourcesOf(target) {
  const [name, page] = target.split('/');
  const app = config.apps[name];
  assert.ok(app, `unknown probe app ${name}`);
  if (app.sources) return app.sources;
  if (!page) return [join(app.project, 'pages')];
  const entry = JSON.parse(readFileSync(join(repo, app.project, 'lxapp.json'), 'utf8')).pages.find(item => item.name === page);
  assert.ok(entry, `${app.project} has no page ${page}`);
  return [join(app.project, entry.path.replace(/\/index$/, ''))];
}

const javaIds = source => [...source.replace(/\b(?:HandlerThread|screenshot)\("[^"]*"/g, '')
  .matchAll(/"([a-z][a-z0-9]*(?:-[A-Za-z0-9]+)+)"|:id\/([a-z][A-Za-z0-9-]*)"/g)]
  .map(match => match[1] ?? match[2]).filter(id => !id.startsWith('gpu-'));
const scriptIds = source => [
  ...source.matchAll(/\b(?:node|tap|center|find)\((?:[^'"()]*(?:\(\))?\s*,\s*)?['"]([a-z][A-Za-z0-9-]*)['"]/g),
  ...source.matchAll(/:id\\?\/([a-z][A-Za-z0-9-]*)/g),
  ...source.matchAll(/test_id\s*===\s*['"]([a-z][A-Za-z0-9-]*)['"]/g),
].map(match => match[1]);

const files = [
  ...walk(join(gpu, 'probes'), /\.java$/),
  ...walk(join(gpu, 'instrumentation'), /\.java$/),
  ...walk(join(gpu, 'scripts'), /\.mjs$/),
  ...walk(join(repo, 'lingxia-sdk/android/tests/gpu/smoke'), /^verify.*\.mjs$/),
];
const nameOf = file => basename(file).replace(/\.java$/, '');

test('every test id a device probe or driver references exists in its target app', () => {
  const sets = new Map(), stale = [];
  const notIds = new Set(config.notIds);
  for (const file of files) {
    const name = nameOf(file), target = config.targets[name] ?? config.default;
    if (!sets.has(target)) sets.set(target, idSet(sourcesOf(target)));
    const known = sets.get(target), source = readFileSync(file, 'utf8');
    const exempt = config.exempt[name] ?? {}, inline = new Set(testIdsIn(source));
    const ids = new Set((file.endsWith('.java') ? javaIds(source) : scriptIds(source)).filter(id => !id.endsWith('-')));
    for (const id of ids) {
      if (!notIds.has(id) && !(id in exempt) && !inline.has(id) && !known(id)) stale.push(`${name} (${target}): ${id}`);
    }
    for (const id of Object.keys(exempt)) assert.ok(!known(id), `${name} exempts ${id}, but ${target} defines it`);
  }
  assert.deepEqual(stale, []);
});

test('probe targets name existing probes and pages', () => {
  const names = new Set(files.map(nameOf));
  for (const [name, target] of Object.entries(config.targets)) {
    assert.ok(names.has(name), `probe-targets.json names missing ${name}`);
    for (const dir of sourcesOf(target)) assert.ok(statSync(join(repo, dir)).isDirectory(), dir);
  }
  for (const name of Object.keys(config.exempt)) assert.ok(names.has(name), `exempt names missing ${name}`);
});
