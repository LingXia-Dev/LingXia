import { test } from 'node:test';
import assert from 'node:assert/strict';
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { galleryPackages } from '../../scripts/gallery-packages.mjs';

function root(listed, folders) {
  const dir = mkdtempSync(join(tmpdir(), 'liuli-packages-'));
  mkdirSync(join(dir, 'gallery'));
  writeFileSync(join(dir, 'gallery', 'packages.json'), JSON.stringify(listed));
  for (const name of folders) {
    mkdirSync(join(dir, name));
    writeFileSync(join(dir, name, 'catalog.json'), '{"components":[]}');
  }
  return dir;
}

test('the gallery reads only the packages it lists: a folder beside them with a catalog is ignored', () => {
  const dir = root(['core', 'charts'], ['core', 'charts', 'extra']);
  try {
    assert.deepEqual(galleryPackages(dir, 'core').map(entry => entry.name), ['charts']);
  } finally {
    rmSync(dir, { recursive: true });
  }
});

test('a listed package without a catalog fails the run', () => {
  const dir = root(['core', 'charts'], ['core']);
  try {
    assert.throws(() => galleryPackages(dir, 'core'), /lists charts, which has no catalog\.json/);
  } finally {
    rmSync(dir, { recursive: true });
  }
});

test('the tracked list names core and charts', () => {
  assert.deepEqual(galleryPackages(new URL('../../..', import.meta.url).pathname, 'core').map(entry => entry.name), ['charts']);
});
