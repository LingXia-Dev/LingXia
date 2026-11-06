// Renders View pages headlessly through `examples/corpus.rs` at phone and tablet size in both
// appearances, in two lanes:
//   fixture: crates/lingxia-view/tests/fixtures, one renderer construct per page; cost budgets and
//     an exact SHA-256 of each CPU frame.
//   showcase: every GPU page of examples/lingxia-showcase-gpu with its captured Logic state; cost
//     budgets and UI_* errors only, since its pixels follow product edits.
// Each lane keeps budgets.json (and the fixture lane renders.json) beside its pages. Timings are informative.
//   node corpus/run.mjs [--lane fixture|showcase] [--only <substring>] [--update-budgets] [--png] [--nodes]
//   node corpus/run.mjs --visual [--update-visual] [--only <substring>] | --diff <id>
// `--visual` checks fixture hashes only, keyed by platform-arch: the rasterizer picks SIMD paths at
// run time and fonts come from the host. Frames land in target/corpus/frames/, changed ones also in
// target/corpus/changed/; `--diff <id>` writes an id's previous (when kept) and current frame to target/corpus/diff/.
import { readFile, writeFile, mkdir, readdir, rm, copyFile } from 'node:fs/promises';
import { existsSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { execFileSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { buildPages } from '../compiler/index.mjs';

const here = fileURLToPath(new URL('.', import.meta.url));
const repo = resolve(here, '../../..');
const out = join(repo, 'target/corpus');
const args = process.argv.slice(2);
const flag = name => args.includes(name);
const option = name => args.includes(name) ? args[args.indexOf(name) + 1] : undefined;
const diff = option('--diff'), only = option('--only'), laneName = option('--lane');
const visual = flag('--visual') || flag('--update-visual') || diff !== undefined;
const platform = `${process.platform}-${process.arch}`;
const file = id => id.replace(/[/@]/g, '_');
const readJson = async (path, fallback) => existsSync(path) ? JSON.parse(await readFile(path, 'utf8')) : fallback;

const fixtures = join(repo, 'crates/lingxia-view/tests/fixtures');
const showcase = join(repo, 'examples/lingxia-showcase-gpu/lxapp');
const lanes = [
  { name: 'fixture', root: fixtures, states: fixtures, budgets: join(fixtures, 'budgets.json'), renders: join(fixtures, 'renders.json'), images: true,
    pages: async () => (await readdir(fixtures)).filter(name => name.endsWith('.tsx')).sort() },
  { name: 'showcase', root: showcase, states: join(showcase, 'tests/corpus/states'), budgets: join(showcase, 'tests/corpus/budgets.json'),
    pages: () => gpuPages(showcase) },
].filter(lane => (!laneName || lane.name === laneName) && (!visual || lane.renders));
const viewports = [
  { name: 'phone', width: 393, height: 852, scale: 2 },
  { name: 'tablet', width: 820, height: 1180, scale: 2 },
];

async function gpuPages(root) {
  const manifest = JSON.parse(await readFile(join(root, 'lxapp.json'), 'utf8'));
  if (manifest.renderer !== 'gpu') return [];
  return (manifest.pages ?? []).map(page => `${page.path.replace(/\.tsx$/, '')}.tsx`).filter(page => existsSync(join(root, page)));
}

const pageName = page => page.replace(/^pages\//, '').replace(/\/index\.tsx$/, '').replace(/\.tsx$/, '');

/** `<page>.json` is a page's initial state (empty without one); `<page>.<variant>.json` adds another. */
async function states(dir, name) {
  const files = existsSync(dir) ? (await readdir(dir)).filter(file => file === `${name}.json` || file.startsWith(`${name}.`) && file.endsWith('.json')) : [];
  const found = await Promise.all(files.sort().map(async file => ({
    variant: file === `${name}.json` ? 'initial' : file.slice(name.length + 1, -5),
    state: JSON.parse(await readFile(join(dir, file), 'utf8')),
  })));
  return found.some(state => state.variant === 'initial') ? found : [{ variant: 'initial', state: {} }, ...found];
}

const jobs = [], problems = [];
for (const lane of lanes) {
  const pages = await lane.pages();
  const dist = join(out, lane.name);
  try { await buildPages(lane.root, pages, dist); }
  catch (error) { problems.push(`${lane.name}: build failed\n${error.message}`); continue; }
  for (const page of pages) {
    const name = pageName(page);
    for (const { variant, state } of await states(lane.states, name))
      for (const viewport of viewports)
        for (const theme of ['light', 'dark']) {
          const id = `${lane.name}/${name}${variant === 'initial' ? '' : `.${variant}`}@${viewport.name}-${theme}`;
          if (diff ? id !== diff : only && !id.includes(only)) continue;
          jobs.push({ id, module: join(dist, page.replace(/\.tsx$/, '.js')), width: viewport.width, height: viewport.height,
            scale: viewport.scale, dark: theme === 'dark', locale: state.__locale ?? 'en', state,
            ...(lane.images ? { root: lane.root } : {}),
            ...(visual ? { png: join(out, 'frames', `${file(id)}.png`), hash: true }
              : flag('--png') ? { png: join(out, 'png', `${file(id)}.png`) } : {}),
            ...(flag('--nodes') ? { nodes: join(out, 'nodes', `${file(id)}.json`) } : {}) });
        }
  }
}
if (diff !== undefined && !jobs.length) problems.push(`--diff ${diff}: no such fixture id`);
await mkdir(join(out, 'png'), { recursive: true });
await mkdir(join(out, 'nodes'), { recursive: true });
if (visual) {
  await mkdir(join(out, 'frames'), { recursive: true });
  await rm(join(out, 'changed'), { recursive: true, force: true });
  await mkdir(join(out, 'changed'), { recursive: true });
}
if (diff !== undefined) {
  await rm(join(out, 'diff'), { recursive: true, force: true });
  await mkdir(join(out, 'diff'), { recursive: true });
  if (existsSync(join(out, 'frames', `${file(diff)}.png`)))
    await copyFile(join(out, 'frames', `${file(diff)}.png`), join(out, 'diff', `${file(diff)}.before.png`));
}
const jobsPath = join(out, 'jobs.json'), resultsPath = join(out, 'results.json');
await writeFile(jobsPath, JSON.stringify(jobs));
execFileSync('cargo', ['run', '-q', '-p', 'lingxia-view', '--example', 'corpus', '--', jobsPath, resultsPath], { cwd: repo, stdio: 'inherit' });
const results = JSON.parse(await readFile(resultsPath, 'utf8'));

const counters = result => ({ ...result.cost, layoutPasses: result.layoutPasses, viewPasses: result.viewPasses, wireValues: result.wireValues });
// budgets.json: { "<page>[.<variant>]@<viewport>-<theme>": counters }; renders.json: { "<platform>-<arch>": { id: sha256 } }.
const recorded = {}, measured = {};
for (const lane of lanes)
  recorded[lane.name] = { budgets: await readJson(lane.budgets, {}), renders: lane.renders ? await readJson(lane.renders, {}) : {} };
for (const result of results) {
  if (result.error) { problems.push(`${result.id}: ${result.error}`); continue; }
  const lane = result.id.slice(0, result.id.indexOf('/')), key = result.id.slice(lane.length + 1);
  const seen = (measured[lane] ??= { budgets: {}, renders: {} });
  if (visual) {
    seen.renders[key] = result.hash;
    const hashes = recorded[lane].renders[platform];
    const frame = join(out, 'frames', `${file(result.id)}.png`);
    if (diff !== undefined) await copyFile(frame, join(out, 'diff', `${file(result.id)}.current.png`));
    if (flag('--update-visual')) continue;
    if (!hashes) problems.push(`${result.id}: no hashes recorded for ${platform}; run with --update-visual on such a host`);
    else if (!hashes[key]) problems.push(`${result.id}: no recorded hash; run with --update-visual`);
    else if (hashes[key] !== result.hash) {
      problems.push(`${result.id}: render changed (frame in target/corpus/changed; --diff ${result.id} to compare)`);
      await copyFile(frame, join(out, 'changed', `${file(result.id)}.png`));
    }
    continue;
  }
  const counts = seen.budgets[key] = counters(result);
  const budget = recorded[lane].budgets[key];
  if (!budget) { if (!flag('--update-budgets')) problems.push(`${result.id}: no budget; run with --update-budgets`); continue; }
  for (const [counter, value] of Object.entries(counts))
    if (value > (budget[counter] ?? 0)) problems.push(`${result.id}: ${counter} ${value} exceeds its budget ${budget[counter] ?? 0}`);
}
const sorted = object => Object.fromEntries(Object.entries(object).sort());
// A full run replaces what it records, dropping pages that are gone; --only merges.
const merge = (previous, current) => sorted(only || diff ? { ...previous, ...current } : current);
for (const lane of lanes) {
  const seen = measured[lane.name];
  if (!seen) continue;
  if (flag('--update-budgets')) await writeFile(lane.budgets, JSON.stringify(merge(recorded[lane.name].budgets, seen.budgets), null, 1) + '\n');
  if (flag('--update-visual')) {
    const renders = recorded[lane.name].renders;
    await writeFile(lane.renders, JSON.stringify(sorted({ ...renders, [platform]: merge(renders[platform] ?? {}, seen.renders) }), null, 1) + '\n');
  }
}
if (visual) await writeFile(join(out, 'renders.json'), JSON.stringify({ platform, renders: Object.fromEntries(Object.entries(measured).map(([lane, seen]) => [lane, sorted(seen.renders)])) }, null, 1) + '\n');
await writeFile(join(out, 'report.json'), JSON.stringify({ results, problems }, null, 2));
const slowest = [...results].filter(r => !r.error).sort((a, b) => b.medianUs - a.medianUs).slice(0, 5);
console.log(`corpus: ${results.length} renders, ${problems.length} problems; slowest (informative): ${slowest.map(r => `${r.id} ${(r.medianUs / 1000).toFixed(1)}ms`).join(', ')}`);
for (const problem of problems) console.log(`  ${problem}`);
process.exit(problems.length ? 1 : 0);
