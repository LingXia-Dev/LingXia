import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { createRequire } from 'node:module';
const postcss = createRequire(new URL('../../../packages/lingxia-ui/package.json', import.meta.url))('postcss');

test('GPU showcase tokens hold scales only; colours come from theme roles', async () => {
  const props = [];
  postcss.parse(await readFile(new URL('../lxapp/theme/tokens.css', import.meta.url), 'utf8')).walkDecls(decl => props.push(decl.prop));
  assert.deepEqual(props.filter(prop => prop.startsWith('--color-')), []);
  assert.deepEqual(props.filter(prop => prop.startsWith('--lx-') && !prop.startsWith('--lx-shadow')), []);
});
