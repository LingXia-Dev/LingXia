import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { renderPage } from './testing/harness.mjs';

test('the package root gives each component, the nav hook, the provider and extractOutline; tokens.css has a path', async () => {
  const { exports } = JSON.parse(readFileSync(new URL('../package.json', import.meta.url), 'utf8'));
  assert.equal(exports['.'], './src/index.ts');
  assert.equal(exports['./tokens.css'], './tokens.css');
  const screen = await renderPage(`
    import { JinshuDiff, JinshuEditor, JinshuOutline, JinshuProvider, JinshuView, extractOutline, useJinshuNav } from '@liulidesign/jinshu';
    import { createDocument, heading, paragraph } from '@liulidesign/jinshu/schema';
    const doc = createDocument({ title: 'W', blocks: [heading(1, 'Limits', 'h'), paragraph('Text.', 'p')] });
    export function render() {
      const nav = useJinshuNav();
      const given = [JinshuDiff, JinshuEditor, JinshuProvider].every(part => typeof part === 'function');
      return (
        <page>
          <JinshuOutline items={extractOutline(doc)} nav={nav} title={given ? 'Contents' : 'missing'} />
          <JinshuView doc={doc} nav={nav} />
        </page>
      );
    }`);
  assert.ok(screen.queryAll(node => node.props.accessibilityLabel === 'Contents').length > 0);
});
