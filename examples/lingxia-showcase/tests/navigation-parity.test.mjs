import test from 'node:test';
import assert from 'node:assert/strict';
import { readFile, readdir } from 'node:fs/promises';
import { readReference } from './reference.mjs';
const gpu = new URL('../lxapp/', import.meta.url);
const json = async url => JSON.parse(await readFile(url, 'utf8'));
// Renderer fixtures with no React counterpart, and why each exists.
export const gpuOnly = new Map([
  ['view-lab', 'the View renderer kitchen sink'],
  ['motion-lab', 'transitions and motion, a tab'],
  ['motion-target', 'motion-lab as a pushable page: navigateTo refuses tab pages'],
  ['perf-lab', 'a standard workload per pipeline stage, a tab'],
  ['perf-target', 'the page perf-lab pushes and measures'],
]);
// Pages whose title differs from the reference, and why; the rest of their bar matches.
const retitled = new Map([
  ['video', 'a View <video>, not the native video component the reference names'],
]);
// Reference tabs the GPU app leaves out of its bar, and why; their pages stay pushable.
const droppedTabs = new Map([
  ['components', 'the native component gallery has no GPU counterpart'],
  ['media', 'gave its tab to Motion Lab'],
  ['device', 'gave its tab to Perf Lab'],
]);

test('GPU pages preserve the reference host navigation configuration', async () => {
  // The renderer is declared once, by the app; a page config naming one fails the build.
  assert.equal((await json(new URL('lxapp.json', gpu))).renderer, 'gpu');
  for (const name of await readdir(new URL('pages/', gpu))) {
    const path = `pages/${name}/index.json`;
    const actual = await json(new URL(path, gpu));
    assert.equal(actual.renderer, undefined, name);
    if (gpuOnly.has(name)) continue;
    // The reference keeps its uppercase API directory.
    const expected = JSON.parse(readReference(path.replace('pages/api/', 'pages/API/')));
    assert.equal(actual.navigationStyle ?? 'default', expected.navigationStyle ?? 'default', name);
    const bar = navigationBar => (retitled.has(name) ? { ...navigationBar, title: undefined } : navigationBar);
    assert.deepEqual(bar(actual.navigationBar), bar(expected.navigationBar), name);
  }
});

test('host tabs match the reference except the dropped tabs', async () => {
  const actual = (await json(new URL('lxapp.json', gpu))).tabBar;
  const expected = JSON.parse(readReference('lxapp.json')).tabBar;
  expected.items = expected.items.filter(item => !droppedTabs.has(item.page));
  // Icons and the unselected colour follow legibility, not the reference: raster
  // icons cannot take the style tint, so tab glyphs are SVG templates. Media is
  // mobile-only here (`showOn`).
  const shape = ({ style, items, ...rest }) => ({ ...rest, items: items.map(({ iconPath, showOn, ...item }) => item) });
  // Renderer fixtures may add their own tabs.
  assert.deepEqual(shape({ ...actual, items: actual.items.filter(item => !gpuOnly.has(item.page)) }), shape(expected));
  assert.equal(actual.style.selectedForegroundColor, expected.style.selectedForegroundColor);
  for (const item of actual.items) {
    assert.match(item.iconPath, /\.svg$/);
    await readFile(new URL(item.iconPath, gpu));
  }
});
