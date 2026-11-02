// The View corpus: compiles real pages, renders each at phone and tablet size in both
// appearances through `examples/corpus.rs`, and fails on any UI_* error or on a deterministic
// cost counter above its budget in budgets.json. Timings are informative only.
//   node corpus/run.mjs [--update-budgets] [--png] [--nodes] [--only <substring>]
//   node corpus/run.mjs --visual [--update-visual] [--only <substring>]
// `--visual` compares each render's fingerprint (16-px block means) with visual/<id>.png and writes
// the frame with its changed blocks outlined to target/corpus/visual-diff/; it checks no budgets.
import { readFile, writeFile, mkdir, readdir, rm } from 'node:fs/promises';
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
const only = args.includes('--only') ? args[args.indexOf('--only') + 1] : undefined;

/** Apps whose GPU pages the corpus covers; states/<app>/<page>[.<variant>].json feed Logic data. */
const apps = [
  { name: 'showcase-gpu', root: join(repo, 'examples/lingxia-showcase-gpu/lxapp') },
  { name: 'corpus', root: join(here, 'app') },
];
const viewports = [
  { name: 'phone', width: 393, height: 852, scale: 2 },
  { name: 'tablet', width: 820, height: 1180, scale: 2 },
];

async function gpuPages(root) {
  const manifest = JSON.parse(await readFile(join(root, 'lxapp.json'), 'utf8'));
  const pages = [];
  if (manifest.renderer !== 'gpu') return pages;
  for (const page of manifest.pages ?? []) {
    const path = page.path.replace(/\.tsx$/, '');
    if (existsSync(join(root, `${path}.tsx`))) pages.push(`${path}.tsx`);
  }
  return pages;
}

async function states(app, page) {
  const dir = join(here, 'states', app);
  const name = page.replace(/^pages\//, '').replace(/\/index\.tsx$/, '');
  if (!existsSync(dir)) return [{ variant: 'initial', state: {} }];
  const files = (await readdir(dir)).filter(file => file === `${name}.json` || file.startsWith(`${name}.`) && file.endsWith('.json'));
  const found = await Promise.all(files.sort().map(async file => ({
    variant: file === `${name}.json` ? 'initial' : file.slice(name.length + 1, -5),
    state: JSON.parse(await readFile(join(dir, file), 'utf8')),
  })));
  return found.length ? found : [{ variant: 'initial', state: {} }];
}

const jobs = [], problems = [];
const visual = flag('--visual') || flag('--update-visual');
for (const app of apps) {
  if (!existsSync(join(app.root, 'lxapp.json'))) continue;
  const pages = await gpuPages(app.root);
  const dist = join(out, app.name);
  try { await buildPages(app.root, pages, dist); }
  catch (error) { problems.push(`${app.name}: build failed\n${error.message}`); continue; }
  for (const page of pages) {
    const name = page.replace(/^pages\//, '').replace(/\/index\.tsx$/, '');
    for (const { variant, state } of await states(app.name, page))
      for (const viewport of viewports)
        for (const theme of ['light', 'dark']) {
          const id = `${app.name}/${name}${variant === 'initial' ? '' : `.${variant}`}@${viewport.name}-${theme}`;
          if (only && !id.includes(only)) continue;
          jobs.push({ id, module: join(dist, page.replace(/\.tsx$/, '.js')), width: viewport.width, height: viewport.height,
            scale: viewport.scale, dark: theme === 'dark', locale: state.__locale ?? 'en', state,
            ...(flag('--png') ? { png: join(out, 'png', `${id.replace(/[/@]/g, '_')}.png`) } : {}),
            ...(flag('--nodes') ? { nodes: join(out, 'nodes', `${id.replace(/[/@]/g, '_')}.json`) } : {}),
            ...(visual ? { visual: { reference: join(here, 'visual', `${id.replace(/[/@]/g, '_')}.png`),
              diff: join(out, 'visual-diff', `${id.replace(/[/@]/g, '_')}.png`), update: flag('--update-visual') } } : {}) });
        }
  }
}
await mkdir(join(out, 'png'), { recursive: true });
await mkdir(join(out, 'nodes'), { recursive: true });
if (visual) {
  await mkdir(join(here, 'visual'), { recursive: true });
  await rm(join(out, 'visual-diff'), { recursive: true, force: true });
  await mkdir(join(out, 'visual-diff'), { recursive: true });
}
const jobsPath = join(out, 'jobs.json'), resultsPath = join(out, 'results.json');
await writeFile(jobsPath, JSON.stringify(jobs));
execFileSync('cargo', ['run', '-q', '-p', 'lingxia-ui', '--example', 'corpus', '--', jobsPath, resultsPath], { cwd: repo, stdio: 'inherit' });
const results = JSON.parse(await readFile(resultsPath, 'utf8'));

const budgetsPath = join(here, 'budgets.json');
const budgets = existsSync(budgetsPath) ? JSON.parse(await readFile(budgetsPath, 'utf8')) : {};
const counters = result => ({ ...result.cost, layoutPasses: result.layoutPasses, viewPasses: result.viewPasses, wireValues: result.wireValues });
const next = {};
for (const result of results) {
  if (result.error) { problems.push(`${result.id}: ${result.error}`); continue; }
  if (visual) {
    const seen = result.visual ?? {};
    if (seen.missing) problems.push(`${result.id}: no visual reference; run with --update-visual`);
    else if (seen.changed) problems.push(`${result.id}: ${seen.changed} blocks differ from visual/${result.id.replace(/[/@]/g, '_')}.png (outlined in target/corpus/visual-diff)`);
    continue;
  }
  const measured = counters(result);
  next[result.id] = measured;
  const budget = budgets[result.id];
  if (!budget) { if (!flag('--update-budgets')) problems.push(`${result.id}: no budget; run with --update-budgets`); continue; }
  for (const [counter, value] of Object.entries(measured))
    if (value > (budget[counter] ?? 0)) problems.push(`${result.id}: ${counter} ${value} exceeds its budget ${budget[counter] ?? 0}`);
}
if (flag('--update-budgets')) await writeFile(budgetsPath, JSON.stringify(Object.fromEntries(Object.entries({ ...budgets, ...next }).sort()), null, 1) + '\n');
await writeFile(join(out, 'report.json'), JSON.stringify({ results, problems }, null, 2));
const slowest = [...results].filter(r => !r.error).sort((a, b) => b.medianUs - a.medianUs).slice(0, 5);
console.log(`corpus: ${results.length} renders, ${problems.length} problems; slowest (informative): ${slowest.map(r => `${r.id} ${(r.medianUs / 1000).toFixed(1)}ms`).join(', ')}`);
for (const problem of problems) console.log(`  ${problem}`);
process.exit(problems.length ? 1 : 0);
