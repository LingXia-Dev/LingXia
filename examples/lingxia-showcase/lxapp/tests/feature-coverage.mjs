// Page-feature coverage gate for both showcases. Every page or app config
// feature they use (pull to refresh, navigation bar styles, orientation, tab
// bar, …) and every interactive GPU View element kind must be claimed with
// `covers: ['page:…' | 'app:…' | 'view:…']` by a spec that drives it the way
// a user does (a tap, typing or a gesture). Unclaimed ids fail unless
// feature-gaps.json names them with a reason.
//
//   node tests/feature-coverage.mjs                    # from the sources (CI)
//   node tests/feature-coverage.mjs --report R.json --app gpu
//       # a device run: only passed cases that recorded a user action count
import { existsSync, readFileSync, readdirSync, statSync } from 'node:fs';
import { dirname, join, relative } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const examples = join(here, '../../..');
const repo = join(examples, '..');
const APPS = {
  react: { lxapp: join(examples, 'lingxia-showcase/lxapp') },
  gpu: { lxapp: join(examples, 'lingxia-showcase-gpu/lxapp') },
};
const USER_PATH = /\.(click|dblclick|fill|type|press|selectOption|swipe|drag|fling|pullToRefresh|touch|tap)\(|\bturn\(/;
const ACTION_STEPS = /^(page\.(click|dblclick|fill|type|press|selectOption)|gesture\.|finger\.|chrome\.tap|device\.turn)/;

const json = (path) => JSON.parse(readFileSync(path, 'utf8'));

/** Config features one lxapp uses, as covers ids. */
function configFeatures(lxapp) {
  const ids = new Set();
  const app = json(join(lxapp, 'lxapp.json'));
  if (app.tabBar) {
    ids.add('app:tabBar');
    if (app.tabBar.presentation) ids.add(`app:tabBar.presentation=${app.tabBar.presentation}`);
    if (app.tabBar.style) ids.add('app:tabBar.style');
  }
  for (const page of app.pages ?? []) {
    const config = join(lxapp, dirname(page.path), 'index.json');
    if (!existsSync(config)) continue;
    const c = json(config);
    if (c.enablePullDownRefresh) ids.add('page:enablePullDownRefresh');
    if (c.navigationStyle) ids.add(`page:navigationStyle=${c.navigationStyle}`);
    if (c.navigationBar?.style) ids.add('page:navigationBar.style');
    if (c.pageOrientation) ids.add(`page:pageOrientation=${c.pageOrientation}`);
  }
  return ids;
}

/** Interactive GPU View element kinds, from the View contract. */
function viewKinds() {
  const schema = json(join(repo, 'contracts/view/schema.json'));
  return new Set(Object.entries(schema.elements)
    .filter(([name, element]) => element.binding || ['button', 'link'].includes(name)
      || (element.style ?? []).some((group) => group === 'scroll' || group === 'overscroll'))
    .map(([name]) => `view:${name}`));
}

function testFiles(dir) {
  return readdirSync(dir).flatMap((name) => {
    const path = join(dir, name);
    if (name === 'node_modules' || name === 'fixtures') return [];
    if (statSync(path).isDirectory()) return testFiles(path);
    return name.endsWith('.test.ts') ? [path] : [];
  });
}

/** Spec files each app runs: its own, plus the shared ones the GPU showcase does not skip. */
function specFiles(app) {
  const own = testFiles(join(APPS[app].lxapp, 'tests'));
  if (app !== 'gpu') return own;
  const manifest = json(join(APPS.gpu.lxapp, 'tests/shared-specs.json'));
  const source = join(APPS.gpu.lxapp, 'tests', manifest.source);
  const shared = ['pages', 'flows'].flatMap((dir) => testFiles(join(source, dir)))
    .filter((file) => !(relative(source, file).split('\\').join('/') in manifest.skip));
  return [...own, ...shared];
}

/** `covers` ids of the specs in `file` whose bodies drive the UI as a user does. */
function claims(file) {
  const text = readFileSync(file, 'utf8');
  const starts = [...text.matchAll(/^spec(\.\w+)?\(/gm)].map((m) => m.index);
  const out = [];
  starts.forEach((start, i) => {
    const body = text.slice(start, starts[i + 1] ?? text.length);
    const covers = body.match(/covers:\s*\[([^\]]*)\]/);
    if (!covers) return;
    const ids = [...covers[1].matchAll(/'([^']+)'|"([^"]+)"/g)].map((m) => m[1] ?? m[2]);
    out.push({ ids, userPath: USER_PATH.test(body), file });
  });
  return out;
}

function inventory(app) {
  const ids = configFeatures(APPS[app].lxapp);
  if (app === 'gpu') for (const id of viewKinds()) ids.add(id);
  return ids;
}

function covered(app, report) {
  const ids = new Set();
  if (report) {
    for (const c of json(report).cases ?? []) {
      const acted = (c.gestures?.length ?? 0) > 0 || flatten(c.steps ?? []).some((s) => ACTION_STEPS.test(s.name ?? ''));
      if (c.status === 'passed' && acted) for (const id of c.covers ?? []) ids.add(id);
    }
    return ids;
  }
  for (const file of specFiles(app)) {
    for (const claim of claims(file)) if (claim.userPath) for (const id of claim.ids) ids.add(id);
  }
  return ids;
}

function flatten(steps) {
  return steps.flatMap((step) => [step, ...flatten(step.steps ?? [])]);
}

const args = process.argv.slice(2);
const flag = (name) => { const i = args.indexOf(name); return i >= 0 ? args[i + 1] : undefined; };
const report = flag('--report');
const apps = flag('--app') ? [flag('--app')] : Object.keys(APPS);
const gaps = json(join(here, 'feature-gaps.json')).gaps;
let failed = false;
for (const app of apps) {
  if (!APPS[app]) throw new Error(`--app must be ${Object.keys(APPS).join(' or ')}`);
  const need = inventory(app);
  const have = covered(app, report);
  const missing = [...need].filter((id) => !have.has(id)).sort();
  const excused = missing.filter((id) => gaps[app]?.[id]);
  const open = missing.filter((id) => !gaps[app]?.[id]);
  for (const [id, reason] of Object.entries(gaps[app] ?? {})) {
    if (!reason?.trim()) { console.error(`${app}: gap ${id} has no reason`); failed = true; }
    if (have.has(id)) { console.error(`${app}: gap ${id} is covered now; remove it from feature-gaps.json`); failed = true; }
  }
  console.log(`${app}: ${need.size - missing.length}/${need.size} features driven by a spec${report ? ' that passed' : ''}; ${excused.length} known gaps`);
  for (const id of excused) console.log(`  gap ${id}: ${gaps[app][id]}`);
  for (const id of open) console.error(`  UNCOVERED ${id}`);
  if (open.length > 0) failed = true;
}
process.exit(failed ? 1 : 0);
