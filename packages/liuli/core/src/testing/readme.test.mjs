import { test } from 'node:test';
import assert from 'node:assert/strict';
import { existsSync, readFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { renderPage } from './harness.mjs';

// The README is the first thing a stranger reads: what it shows must compile, and what it names must exist.
const core = resolve(dirname(fileURLToPath(import.meta.url)), '../..');
const readme = readFileSync(join(core, 'README.md'), 'utf8');
const pkg = JSON.parse(readFileSync(join(core, 'package.json'), 'utf8'));

test('README: every page it shows compiles and renders', async () => {
  const pages = [...readme.matchAll(/```tsx\n([\s\S]*?)```/g)].map(match => match[1]).filter(code => /export function render\b/.test(code));
  assert.ok(pages.length >= 1, 'it shows at least one whole page');
  for (const page of pages) await renderPage(page);
});

test('README: every import path it names is exported, and every path and script exists', () => {
  const exported = name => Object.keys(pkg.exports).some(key => key === `./${name}` || (key.endsWith('/*') && name.startsWith(key.slice(2, -1))));
  for (const [, name] of readme.matchAll(/`@liulidesign\/core\/([A-Za-z/]+)`/g)) assert.ok(exported(name), `@liulidesign/core/${name}`);
  for (const [, path] of readme.matchAll(/`(\.\.\/[\w./-]+|docs\/[\w./-]+|[\w-]+\.(?:json|css|md))`/g))
    if (!path.includes('<')) assert.ok(existsSync(join(core, path)) || existsSync(join(core, '../gallery', path)) || /^(lxapp|pack)\.json$|^theme\//.test(path), path);
  for (const [, script] of readme.matchAll(/^npm run ([\w:-]+)/gm)) assert.ok(script in pkg.scripts, `npm run ${script}`);
});
