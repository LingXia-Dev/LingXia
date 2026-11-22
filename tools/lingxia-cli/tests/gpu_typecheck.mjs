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

for (const dir of lxapps) missingBase(path.resolve(dir));

fs.rmSync(root, { recursive: true, force: true });
console.log(`GPU type-check wiring: scaffold and ${lxapps.length} lxapp(s) pass`);
