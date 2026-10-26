import test from 'node:test';
import assert from 'node:assert/strict';
import { readFile, readdir } from 'node:fs/promises';
import { readReference } from './reference.mjs';
const gpu = new URL('../lxapp/', import.meta.url);
const json = async url => JSON.parse(await readFile(url, 'utf8'));
// Renderer fixtures with no React counterpart.
export const gpuOnly = new Set(['view-lab']);

test('GPU pages preserve the reference host navigation configuration', async () => {
  for (const name of await readdir(new URL('pages/', gpu))) {
    const path = `pages/${name}/index.json`;
    const actual = await json(new URL(path, gpu));
    if (gpuOnly.has(name)) { assert.equal(actual.renderer, 'gpu', name); continue; }
    // The reference keeps its uppercase API directory.
    const expected = JSON.parse(readReference(path.replace('pages/api/', 'pages/API/')));
    assert.equal(actual.navigationStyle ?? 'default', expected.navigationStyle ?? 'default', name);
    assert.deepEqual(actual.navigationBar, expected.navigationBar, name);
    assert.equal(actual.renderer, 'gpu', name);
  }
});

test('host tabs match the reference except the excluded component gallery', async () => {
  const actual = (await json(new URL('lxapp.json', gpu))).tabBar;
  const expected = JSON.parse(readReference('lxapp.json')).tabBar;
  expected.items = expected.items.filter(item => item.page !== 'components');
  // Icons and the unselected colour follow legibility, not the reference: raster
  // icons cannot take the style tint, so tab glyphs are SVG templates.
  const shape = ({ style, items, ...rest }) => ({ ...rest, items: items.map(({ iconPath, ...item }) => item) });
  assert.deepEqual(shape(actual), shape(expected));
  assert.equal(actual.style.selectedForegroundColor, expected.style.selectedForegroundColor);
  for (const item of actual.items) {
    assert.match(item.iconPath, /\.svg$/);
    await readFile(new URL(item.iconPath, gpu));
  }
});
