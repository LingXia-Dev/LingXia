// Keeps tests/shared.test.ts, shared-specs.json and the React showcase's
// page and flow specs in step: every source spec is imported, every skip
// names an imported file and gives a reason.
import { readFileSync, readdirSync, statSync } from 'node:fs';
import { dirname, join, relative } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const manifest = JSON.parse(readFileSync(join(here, 'shared-specs.json'), 'utf8'));
const source = join(here, manifest.source);
const specs = ['pages', 'flows'].flatMap((dir) =>
  readdirSync(join(source, dir)).filter((name) => name.endsWith('.test.ts')).map((name) => `${dir}/${name}`));
const entry = readFileSync(join(here, 'shared.test.ts'), 'utf8');
const prefix = relative(here, source).split('\\').join('/');
const imported = [...entry.matchAll(/^import '([^']+)';$/gm)].map((m) => m[1])
  .filter((path) => path.startsWith(`${prefix}/`))
  .map((path) => path.slice(prefix.length + 1).replace(/\.js$/, '.ts'));

const problems = [];
for (const spec of specs) if (!imported.includes(spec)) problems.push(`${spec} is not imported by tests/shared.test.ts`);
for (const file of imported) if (!specs.includes(file)) problems.push(`tests/shared.test.ts imports ${file}, which is not a page or flow spec`);
for (const [file, reason] of Object.entries(manifest.skip)) {
  if (!specs.includes(file)) problems.push(`skip names ${file}, which is not a page or flow spec`);
  if (typeof reason !== 'string' || !reason.trim()) problems.push(`skip of ${file} has no reason`);
}
// Every literal testId a run spec names must exist in both lxapps, so the
// GPU mirror cannot drift from the React pages.
const sourceIds = (root, attr, computed = false) => {
  const ids = new Set();
  const prefixes = [];
  const walk = (dir) => {
    for (const name of readdirSync(dir)) {
      const path = join(dir, name);
      if (name === 'node_modules' || name === 'dist' || name === 'tests') continue;
      if (statSync(path).isDirectory()) walk(path);
      else if (/\.(tsx?|jsx?)$/.test(name)) {
        const text = readFileSync(path, 'utf8');
        for (const m of text.matchAll(new RegExp(`${attr}=(?:"([^"]+)"|\\{'([^']+)'\\}|\\{"([^"]+)"\\})`, 'g'))) ids.add(m[1] ?? m[2] ?? m[3]);
        for (const m of text.matchAll(new RegExp(`${attr}=\\{\`([^\`$]*)\\$\\{`, 'g'))) prefixes.push(m[1]);
        // Ids passed as props to shared components (`testId="x"` on a wrapper).
        for (const m of text.matchAll(/testId=(?:"([^"]+)"|\{'([^']+)'\})/g)) ids.add(m[1] ?? m[2]);
        // Ids computed before they reach `data-testid` (a list's row keys).
        if (computed) {
          for (const m of text.matchAll(/'([a-z][a-z0-9]*(?:-[a-z0-9]+)+)'/g)) ids.add(m[1]);
          for (const m of text.matchAll(/`([a-z][a-z0-9]*(?:-[a-z0-9]+)*-)\$\{/g)) prefixes.push(m[1]);
        }
        // A segmented control names each option `<testId>-<value>`.
        for (const m of text.matchAll(/<Segmented\s[^>]*?testId="([^"]+)"/g)) prefixes.push(`${m[1]}-`);
      }
    }
  };
  walk(root);
  return (id) => ids.has(id) || prefixes.some((prefix) => prefix && id.startsWith(prefix));
};
const lxapps = { react: join(source, '../..'), gpu: join(here, '..') };
const has = { react: sourceIds(join(lxapps.react, 'lxapp'), 'data-testid'), gpu: sourceIds(lxapps.gpu, 'data-testid', true) };
for (const [id, reason] of Object.entries(manifest.keyed ?? {})) {
  if (typeof reason !== 'string' || !reason.trim()) problems.push(`keyed id ${id} has no reason`);
  if (!has.gpu(`${id}-`)) problems.push(`keyed id ${id}: the gpu showcase renders no ${id}-<key>`);
}
for (const spec of specs.filter((file) => !(file in manifest.skip))) {
  const text = readFileSync(join(source, spec), 'utf8');
  const named = new Set([
    ...[...text.matchAll(/testId\(\s*['"]([^'"]+)['"]/g)].map((m) => m[1]),
    ...[...text.matchAll(/data-testid=\\?["']([^"'\\]+)\\?["']/g)].map((m) => m[1]),
  ]);
  for (const id of named) {
    // `lx-*` ids belong to the runtime's own overlays (toast, modal, action
    // sheet), which carry them on both renderers.
    if (id.startsWith('lx-')) continue;
    for (const app of ['react', 'gpu']) {
      if (app === 'gpu' && id in manifest.keyed) continue;
      if (!has[app](id)) problems.push(`${spec} names testId ${id}, which the ${app} showcase does not render`);
    }
  }
}
if (problems.length > 0) {
  console.error(problems.join('\n'));
  process.exit(1);
}
console.log(`shared specs: ${specs.length} files, ${specs.length - Object.keys(manifest.skip).length} run on GPU pages, ${Object.keys(manifest.skip).length} skipped with a reason`);
