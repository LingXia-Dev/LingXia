import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { createRequire } from 'node:module';
import { readReference } from './reference.mjs';
const postcss = createRequire(new URL('../../../packages/lingxia-ui/package.json', import.meta.url))('postcss');

test('GPU showcase static design tokens match the React reference', async () => {
  const tokens = css => {
    const root = postcss.parse(css);
    const blocks = [];
    for (const node of root.nodes) {
      if (!(node.type === 'atrule' && node.name === 'theme') && !(node.type === 'rule' && [':root', "[data-theme='dark']"].includes(node.selector))) continue;
      blocks.push([node.selector ?? `@theme ${node.params}`, node.nodes.filter(child => child.type === 'decl' && !child.prop.startsWith('--animate-')).map(child => [child.prop, child.value.trim()])]);
    }
    return blocks;
  };
  assert.deepEqual(tokens(await readFile(new URL('../lxapp/theme/tokens.css', import.meta.url), 'utf8')), tokens(readReference('theme/tokens.css')));
});
