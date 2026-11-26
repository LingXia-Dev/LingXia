import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { renderPage } from './harness.mjs';

// What the catalog says a component exports is what an app can import: from the package root, from
// the component's own path, and every other component in its file from a path of its own.
const core = resolve(dirname(fileURLToPath(import.meta.url)), '../..');
const { exports } = JSON.parse(readFileSync(join(core, 'package.json'), 'utf8'));
const { components } = JSON.parse(readFileSync(join(core, 'catalog.json'), 'utf8'));
const isComponent = name => /^[A-Z]/.test(name);

test('exports: the root entry gives every component module\'s exports, the values it re-exports and its types, density and hue, and nothing shared between modules', async () => {
  assert.equal(exports['.'], './src/index.ts');
  const index = readFileSync(join(core, 'src', 'index.ts'), 'utf8');
  for (const { name, exports: names } of components) {
    assert.ok(index.includes(`export { ${names.join(', ')} } from './${name}/${name}';`), `${name}'s exports are in the root entry`);
    assert.ok(index.includes(`export type * from './${name}/${name}';`), `${name}'s types are in the root entry`);
  }
  const root = await renderPage(`
    import { Button, DensityProvider, calendarWeekday, hueFor, iconNames } from '@liulidesign/core';
    export function render() { return <page><DensityProvider value="compact"><Button label={String(calendarWeekday('2026-11-23')) + hueFor('a') + iconNames.length} onClick={() => {}} /></DensityProvider></page>; }`);
  assert.equal(root.queryAll(node => node.tag === 'button').length, 1);
  await assert.rejects(renderPage(`
    import { MenuSize } from '@liulidesign/core';
    export function render() { return <page><text>{String(MenuSize)}</text></page>; }`), /MenuSize/);
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
  return <page><Badge label="New" /><Spinner label="Loading" isLabelHidden /><ItemList label="Rows"><Item label="One" data-testid="one" /></ItemList></page>;
}
`);
  assert.ok(screen.getByTestId('one'));
});
