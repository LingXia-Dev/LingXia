import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { createRequire } from 'node:module';
const postcss = createRequire(new URL('../../../packages/lingxia-view/package.json', import.meta.url))('postcss');

test('GPU showcase stylesheets hold scales and elevation only; colours come from theme roles', async () => {
  const props = [];
  for (const name of ['tokens.css', 'elevation.css'])
    postcss.parse(await readFile(new URL(`../lxapp/theme/${name}`, import.meta.url), 'utf8')).walkDecls(decl => props.push(decl.prop));
  assert.deepEqual(props.filter(prop => prop.startsWith('--color-')), []);
  assert.deepEqual(props.filter(prop => prop.startsWith('--lx-') && !prop.startsWith('--lx-shadow')), []);
});
