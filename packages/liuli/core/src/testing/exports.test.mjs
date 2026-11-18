import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

// What the catalog says a component exports is what an app can import: from the package root, from
// the component's own path, and every other component in its file from a path of its own.
const core = resolve(dirname(fileURLToPath(import.meta.url)), '../..');
const { exports } = JSON.parse(readFileSync(join(core, 'package.json'), 'utf8'));
const { components } = JSON.parse(readFileSync(join(core, 'catalog.json'), 'utf8'));
const isComponent = name => /^[A-Z]/.test(name);

test('exports: the root entry gives every component module', () => {
  assert.equal(exports['.'], './src/index.ts');
  const index = readFileSync(join(core, 'src', 'index.ts'), 'utf8');
  for (const { name } of components) assert.ok(index.includes(`export * from './${name}/${name}';`), `${name} is in the root entry`);
});

test('exports: every component the catalog lists has its own path, to the file that defines it', () => {
  for (const entry of components) {
    const file = `./src/${entry.name}/${entry.name}.tsx`;
    assert.equal(entry.import, `@liulidesign/core/${entry.name}`);
    for (const name of entry.exports.filter(isComponent)) assert.equal(exports[`./${name}`], file, `@liulidesign/core/${name}`);
  }
});

test('exports: no component path the catalog does not know', () => {
  const known = new Set(components.flatMap(entry => entry.exports.filter(isComponent)));
  const paths = Object.keys(exports).filter(key => /^\.\/[A-Z]\w*$/.test(key)).map(key => key.slice(2));
  assert.deepEqual(paths.filter(name => !known.has(name)), []);
});

test('exports: a page imports from the root and from a second component\'s own path', async () => {
  const { renderPage } = await import('./harness.mjs');
  const screen = await renderPage(`import { Badge, Spinner } from '@liulidesign/core';
import { ItemList } from '@liulidesign/core/ItemList';
import { Item } from '@liulidesign/core/Item';
export function render() {
  return <page><Badge label="New" /><Spinner label="Loading" /><ItemList label="Rows"><Item label="One" data-testid="one" /></ItemList></page>;
}
`);
  assert.ok(screen.getByTestId('one'));
});
