// Run after cargo build -p lingxia-cli and building packages/:
// node tools/lingxia-cli/tests/gpu_typecheck.mjs <lingxia> [<lxapp> ...]
// Scaffolds a GPU lxapp and checks its type-check wiring and `lingxia check` / `build` reporting;
// each extra <lxapp> gets the missing-base check.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const [cliArg, ...lxapps] = process.argv.slice(2);
assert(cliArg, 'Expected the lingxia binary');
const cli = path.resolve(cliArg);
const repo = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../../..');
const packages = path.join(repo, 'packages');
const tsc = path.join(packages, 'node_modules/typescript/bin/tsc');
const root = fs.mkdtempSync(path.join(os.tmpdir(), 'lingxia-gpu-typecheck-'));
const env = { ...process.env, HOME: path.join(root, 'home'), USERPROFILE: path.join(root, 'home') };
fs.mkdirSync(env.HOME);

function run(command, args, cwd) {
  const result = spawnSync(command, args, { cwd, env, encoding: 'utf8', timeout: 180_000 });
  assert.ifError(result.error);
  return { status: result.status, output: result.stdout + result.stderr };
}

function ok(command, args, cwd) {
  const result = run(command, args, cwd);
  assert.equal(result.status, 0, `${command} ${args.join(' ')}\n${result.output}`);
  return result.output;
}

const status = dir => ok('git', ['status', '--porcelain', '--untracked-files=all', '--', '.'], dir);

// Before `lingxia gen types`, `tsc -b` fails, names the fix, and writes nothing beside the sources.
function missingBase(dir) {
  const generated = path.join(dir, '.lingxia');
  const aside = path.join(root, `lingxia-${path.basename(dir)}-${Date.now()}`);
  const before = status(dir);
  const hadBase = fs.existsSync(generated);
  if (hadBase) fs.renameSync(generated, aside);
  try {
    const result = run(process.execPath, [tsc, '-b'], dir);
    assert.notEqual(result.status, 0, result.output);
    assert.match(result.output.split('\n').slice(0, 4).join('\n'), /run-lingxia-gen-types/, result.output);
    assert.equal(status(dir), before, `tsc -b wrote files without the base:\n${result.output}`);
  } finally {
    fs.rmSync(generated, { recursive: true, force: true });
    if (hadBase) fs.renameSync(aside, generated);
  }
}

const work = path.join(root, 'work');
fs.mkdirSync(work);
ok(cli, ['new', 'demo', '-t', 'lxapp', '-y'], work);
const app = path.join(work, 'demo');
const modules = path.join(app, 'node_modules');
fs.mkdirSync(path.join(modules, '@lingxia'), { recursive: true });
for (const name of ['view', 'logic', 'types', 'test']) {
  fs.symlinkSync(path.join(packages, `lingxia-${name}`), path.join(modules, '@lingxia', name), 'dir');
}
fs.symlinkSync(path.join(packages, 'node_modules/typescript'), path.join(modules, 'typescript'), 'dir');
if (!fs.existsSync(path.join(app, '.git'))) {
  ok('git', ['init', '-q'], app);
}
ok('git', ['add', '-A'], app);
ok('git', ['-c', 'user.name=t', '-c', 'user.email=t@t', 'commit', '-q', '--allow-empty', '-m', 'scaffold'], app);

missingBase(app);

ok(cli, ['gen', 'types'], app);
ok(process.execPath, [tsc, '-b'], app);

// Logic globals stay out of the View program.
const view = path.join(app, 'pages/home/index.tsx');
const source = fs.readFileSync(view, 'utf8');
fs.writeFileSync(view, `${source}\nexport const leak = () => setTimeout(() => lx.ui.toast({ title: 'x' }), 1);\n`);
try {
  const result = run(process.execPath, [tsc, '-b'], app);
  assert.notEqual(result.status, 0, result.output);
  assert.match(result.output, /index\.tsx.*Cannot find name 'setTimeout'/, result.output);
  assert.match(result.output, /index\.tsx.*Cannot find name 'lx'/, result.output);
} finally {
  fs.writeFileSync(view, source);
}

// `check` and `build` list a View type error, a Logic type error and a View refusal in one run.
assert.match(ok(cli, ['check'], app), /no problems/);
const logic = path.join(app, 'pages/home/index.ts');
const logicSource = fs.readFileSync(logic, 'utf8');
fs.writeFileSync(view, `${source}\nconst viewWrong: number = 'view';\nvoid viewWrong;\nexport const tap = () => lx.ui.toast({ title: 'x' });\n`);
fs.writeFileSync(logic, `${logicSource}\nconst logicWrong: number = 'logic';\nvoid logicWrong;\n`);
try {
  const all = [/pages\/home\/index\.tsx:\d+:\d+: TS2322: Type 'string' is not assignable to type 'number'/,
    /pages\/home\/index\.ts:\d+:\d+: TS2322: Type 'string' is not assignable to type 'number'/,
    /pages\/home\/index\.tsx:\d+:\d+: UI_LX: lx is Logic-only/];
  for (const args of [['check'], ['build']]) {
    const result = run(cli, args, app);
    assert.notEqual(result.status, 0, result.output);
    for (const pattern of all) assert.match(result.output, pattern, `lingxia ${args[0]}\n${result.output}`);
  }
  const skipped = run(cli, ['build', '--no-typecheck'], app);
  assert.notEqual(skipped.status, 0, skipped.output);
  assert.match(skipped.output, /UI_LX/, skipped.output);
  assert.doesNotMatch(skipped.output, /TS2322/, skipped.output);
} finally {
  fs.writeFileSync(view, source);
  fs.writeFileSync(logic, logicSource);
}

// The type-check writes nothing outside `.lingxia/`, whatever the tsconfig: a dev session's watcher
// would take any other write as an edit and check again, forever.
function tree(dir, base = dir, seen = new Map()) {
  for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
    if (['.lingxia', 'node_modules', '.git'].includes(entry.name)) continue;
    const file = path.join(dir, entry.name);
    if (entry.isDirectory()) tree(file, base, seen);
    else seen.set(path.relative(base, file), fs.statSync(file).mtimeMs);
  }
  return seen;
}
const rootConfig = path.join(app, 'tsconfig.json');
const solution = fs.readFileSync(rootConfig, 'utf8');
// Plain: not composite, no tsBuildInfoFile, emitting; `tsc -b` alone would write beside the sources.
fs.writeFileSync(rootConfig, JSON.stringify({
  compilerOptions: { target: 'ES2020', module: 'ESNext', moduleResolution: 'bundler', strict: true, skipLibCheck: true,
    types: ['@lingxia/types', '@lingxia/types/logic-globals'] },
  include: ['lxapp.ts', 'pages/**/index.ts'],
}));
try {
  for (const config of [rootConfig, null]) {
    if (config === null) fs.writeFileSync(rootConfig, solution);
    const before = tree(app);
    assert.match(ok(cli, ['check'], app), /no problems/);
    const after = tree(app);
    assert.deepEqual(after, before, 'the check wrote outside .lingxia/');
    ok(cli, ['check'], app);
    assert.deepEqual(tree(app), after, 'a second check changed a file outside .lingxia/');
  }
} finally {
  fs.writeFileSync(rootConfig, solution);
}

// Sources from outside the project, linked in (a source-distributed package) or path-mapped for its
// types (as the Liuli gallery maps `@lingxia/view`), type-check and build: no `rootDir` narrowing.
const shared = path.join(work, 'shared');
fs.mkdirSync(shared);
fs.writeFileSync(path.join(shared, 'format.ts'), 'export type Label = string;\nexport const label = (n: number): Label => `#${n}`;\n');
const linked = path.join(app, 'linked');
fs.symlinkSync(shared, linked, 'dir');
fs.writeFileSync(rootConfig, JSON.stringify({
  compilerOptions: { target: 'ES2020', module: 'ESNext', moduleResolution: 'bundler', strict: true, skipLibCheck: true,
    noEmit: true, paths: { '@shared/*': ['../shared/*'] }, types: ['@lingxia/types', '@lingxia/types/logic-globals'] },
  include: ['lxapp.ts', 'pages/**/index.ts'],
}));
// A value imported through the `paths` alias reaches the Logic bundle, not only its type.
fs.writeFileSync(logic, `import { label as linkedLabel } from '../../linked/format';\nimport { label as aliasedLabel, type Label } from '@shared/format';\n${logicSource}\nexport const labels: Label[] = [linkedLabel(1), aliasedLabel(2)];\n`);
try {
  assert.match(ok(cli, ['check'], app), /no problems/);
  ok(cli, ['build', '--release'], app);
} finally {
  fs.writeFileSync(rootConfig, solution);
  fs.writeFileSync(logic, logicSource);
  fs.rmSync(linked);
}

// Composed actions are read from their type: a spread and a helper's return build with every name
// and kind; a name only known at run time is refused with the fix.
function catalog() {
  const found = [];
  const walk = dir => {
    for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
      if (['node_modules', '.git', '.lingxia'].includes(entry.name)) continue;
      const file = path.join(dir, entry.name);
      if (entry.isDirectory()) walk(file);
      else if (file.endsWith('.js')) {
        // A release bundle is minified: bare keys and template-literal strings, so read it as JS.
        const match = fs.readFileSync(file, 'utf8').match(/"?__names"?:\[[^\]]*\],"?__modes"?:\{[^}]*\}/);
        if (match) found.push(new Function(`return {${match[0]}}`)());
      }
    }
  };
  walk(app);
  assert.equal(found.length, 1, `one built page catalog, found ${found.length}`);
  return found[0];
}
const helper = path.join(app, 'pages/home/shared.ts');
fs.writeFileSync(helper, `export function shared() {
  return {
    async *greet({ name }: { name: string }) { yield name; },
    reset() {},
  };
}
export const key: string = 'reset';
`);
const composed = [
  ['spread', `import { definePage } from '@lingxia/logic';\nimport { shared } from './shared';\nexport default definePage({ data: { greetCount: 0 }, actions: () => ({ ...shared(), async own() { return 1; } }) });\n`,
    { __names: ['greet', 'reset', 'own'], __modes: { greet: 'stream', reset: 'call', own: 'call' } }],
  ['helper', `import { definePage } from '@lingxia/logic';\nimport { shared } from './shared';\nexport default definePage({ data: { greetCount: 0 }, actions: () => shared() });\n`,
    { __names: ['greet', 'reset'], __modes: { greet: 'stream', reset: 'call' } }],
];
try {
  for (const [name, text, expected] of composed) {
    fs.writeFileSync(logic, text);
    ok(cli, ['build', '--release'], app);
    const built = catalog();
    assert.deepEqual([...built.__names].sort(), [...expected.__names].sort(), name);
    assert.deepEqual(built.__modes, expected.__modes, name);
  }
  fs.writeFileSync(logic, `import { definePage } from '@lingxia/logic';\nimport { key } from './shared';\nexport default definePage({ data: { greetCount: 0 }, actions: { [key]() {} } });\n`);
  const refused = run(cli, ['build', '--release', '--no-typecheck'], app);
  assert.notEqual(refused.status, 0, refused.output);
  assert.match(refused.output, /pages\/home\/index: actions must be statically known: write them in the object, or type the helper's return/, refused.output);
} finally {
  fs.writeFileSync(logic, logicSource);
  fs.rmSync(helper, { force: true });
}

// Composed pages no tsconfig project reaches are named together, with the fix for the project's wiring.
const manifest = path.join(app, 'lxapp.json');
const manifestSource = fs.readFileSync(manifest, 'utf8');
const second = path.join(app, 'pages/second');
const logicConfig = path.join(app, 'tsconfig.logic.json');
const logicConfigSource = fs.readFileSync(logicConfig, 'utf8');
const spread = `import { definePage } from '@lingxia/logic';\nconst shared = { async ping() { return 1; } };\nexport default definePage({ data: { greetCount: 0 }, actions: () => ({ ...shared }) });\n`;
try {
  fs.cpSync(path.join(app, 'pages/home'), second, { recursive: true });
  for (const file of [logic, path.join(second, 'index.ts')]) fs.writeFileSync(file, spread);
  const pages = JSON.parse(manifestSource);
  pages.pages.push({ name: 'second', path: 'pages/second/index' });
  fs.writeFileSync(manifest, JSON.stringify(pages, null, 2));
  const unreached = output => {
    assert.match(output, /2 pages compose their actions, which the build reads with TypeScript, but no tsconfig project includes their Logic modules:/, output);
    assert.match(output, /  pages\/home\/index \(pages[\\/]home[\\/]index\.ts\)\n  pages\/second\/index \(pages[\\/]second[\\/]index\.ts\)/, output);
    assert.doesNotMatch(output, /\.\.\.shared/, output);
  };
  // A Logic tsconfig that includes neither page: add them to its `include`.
  fs.writeFileSync(logicConfig, JSON.stringify({ ...JSON.parse(logicConfigSource), include: [] }));
  let refused = run(cli, ['build', '--release', '--no-typecheck'], app);
  assert.notEqual(refused.status, 0, refused.output);
  unreached(refused.output);
  assert.match(refused.output, /Add them to the Logic tsconfig's `include`\./, refused.output);
  // No Logic tsconfig at all: create one; `lingxia check` says so before a build does.
  fs.rmSync(logicConfig);
  for (const command of [['build', '--release', '--no-typecheck'], ['check']]) {
    refused = run(cli, command, app);
    assert.notEqual(refused.status, 0, refused.output);
    unreached(refused.output);
    assert.match(refused.output, /This project has no Logic tsconfig: create `tsconfig\.logic\.json` with `\{ "extends": "\.\/\.lingxia\/tsconfig\.logic\.json" \}` and add `\{ "path": "\.\/tsconfig\.logic\.json" \}` to the `references` of `tsconfig\.json`\./, refused.output);
  }
} finally {
  fs.writeFileSync(logicConfig, logicConfigSource);
  fs.writeFileSync(manifest, manifestSource);
  fs.writeFileSync(logic, logicSource);
  fs.rmSync(second, { recursive: true, force: true });
}

// A TypeScript outside the supported range is refused with the fix, never driven half-way.
const realTypescript = path.join(modules, 'typescript');
fs.renameSync(realTypescript, `${realTypescript}.real`);
try {
  fs.mkdirSync(path.join(realTypescript, 'bin'), { recursive: true });
  fs.writeFileSync(path.join(realTypescript, 'package.json'), JSON.stringify({ name: 'typescript', version: '7.0.2' }));
  fs.writeFileSync(path.join(realTypescript, 'bin/tsc'), '#!/usr/bin/env node\nprocess.exit(0);\n');
  const refused = run(cli, ['check'], app);
  assert.notEqual(refused.status, 0, refused.output);
  assert.match(refused.output, /TypeScript 7\.0\.2 is installed here; LingXia supports \^5\.9\.3: run `npm i -D typescript@\^5\.9\.3`/, refused.output);
} finally {
  fs.rmSync(realTypescript, { recursive: true, force: true });
  fs.renameSync(`${realTypescript}.real`, realTypescript);
}

for (const dir of lxapps) missingBase(path.resolve(dir));

fs.rmSync(root, { recursive: true, force: true });
console.log(`GPU type-check wiring: scaffold and ${lxapps.length} lxapp(s) pass`);
