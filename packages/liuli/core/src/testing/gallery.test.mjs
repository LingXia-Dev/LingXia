import { test } from 'node:test';
import assert from 'node:assert/strict';
import { existsSync, readFileSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { render } from '@lingxia/view/testing';

const gallery = resolve(dirname(fileURLToPath(import.meta.url)), '../../../gallery');
const pages = JSON.parse(readFileSync(resolve(gallery, 'lxapp.json'), 'utf8')).pages;

// Device tests find a gallery page's elements by test id, so ids must not repeat across its examples.
for (const page of pages) {
  test(`gallery ${page.name}: every test id on the page is unique`, async () => {
    const screen = await render(resolve(gallery, `${page.path}.tsx`), {}, { root: gallery });
    const ids = screen.queryAll(node => Boolean(node.testId)).map(node => node.testId);
    const repeated = ids.filter((id, index) => ids.indexOf(id) !== index);
    assert.deepEqual(repeated, []);
  });
}

// The home page is the gallery's only way in, so every other page needs exactly one tile.
test('gallery home: one tile per page, in manifest order', async () => {
  const screen = await render(resolve(gallery, 'pages/home/index.tsx'), {}, { root: gallery });
  const tiles = screen.queryAll(node => node.testId?.startsWith('open-')).map(node => node.testId.slice('open-'.length));
  assert.deepEqual(tiles, pages.filter(page => page.name !== 'home').map(page => page.name));
});

// The device spec walks the home tiles from its own list; keep it equal to the manifest.
// The device specs are local and untracked, so this is skipped where they are absent.
const deviceSpec = resolve(gallery, 'zz-tests/gallery.test.ts');
test('gallery device spec: walks every tile', { skip: !existsSync(deviceSpec) }, () => {
  const spec = readFileSync(deviceSpec, 'utf8');
  const list = spec.match(/const pages = \[([\s\S]*?)\];/)[1].match(/'([a-z-]+)'/g).map(name => name.slice(1, -1));
  assert.deepEqual(list, pages.filter(page => page.name !== 'home').map(page => page.name));
});
