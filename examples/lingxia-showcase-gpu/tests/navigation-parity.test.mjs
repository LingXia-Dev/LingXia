import test from 'node:test';
import assert from 'node:assert/strict';
import { readFile, readdir } from 'node:fs/promises';
import { readReference } from './reference.mjs';
const gpu = new URL('../lxapp/', import.meta.url);
const json = async url => JSON.parse(await readFile(url, 'utf8'));

test('GPU pages preserve the reference host navigation configuration', async () => {
  for (const name of await readdir(new URL('pages/', gpu))) {
    const path = `pages/${name}/index.json`;
    const actual = await json(new URL(path, gpu));
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
  assert.deepEqual(actual, expected);
  for (const item of actual.items) await readFile(new URL(item.iconPath, gpu));
});
