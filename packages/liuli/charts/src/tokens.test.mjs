import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { test } from 'node:test';

const tokens = file => Object.fromEntries([...readFileSync(new URL(file, import.meta.url), 'utf8').matchAll(/^\s*(--[\w-]+):\s*([^;]+);/gm)].map(match => [match[1], match[2]]));

// A page that bundles both packages fails to build when they differ; this fails first.
test('every token charts declares has the value Liuli core gives it', () => {
  const [own, core] = [tokens('../tokens.css'), tokens('../../core/tokens.css')];
  assert.ok(Object.keys(own).length > 0);
  for (const [name, value] of Object.entries(own)) assert.equal(value, core[name], name);
});

test('charts names no font family', () => {
  assert.doesNotMatch(readFileSync(new URL('../tokens.css', import.meta.url), 'utf8'), /--font-|font-family/);
});
