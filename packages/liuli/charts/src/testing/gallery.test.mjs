import { test } from 'node:test';
import assert from 'node:assert/strict';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { render } from '@lingxia/view/testing';

const gallery = resolve(dirname(fileURLToPath(import.meta.url)), '../../../gallery');
const catalog = (await import(new URL('../../catalog.json', import.meta.url), { with: { type: 'json' } })).default;
const kebab = name => name.replace(/([a-z])([A-Z])/g, '$1-$2').toLowerCase();

// Device tests find a gallery page's elements by test id, so ids must not repeat across its examples.
for (const { name } of catalog.components) {
  test(`gallery ${kebab(name)}: builds inside the gallery, and every test id on the page is unique`, async () => {
    const screen = await render(resolve(gallery, `pages/${kebab(name)}/index.tsx`), {}, { root: gallery });
    const ids = screen.queryAll(node => Boolean(node.testId)).map(node => node.testId);
    assert.ok(ids.length > 1);
    assert.deepEqual(ids.filter((id, index) => ids.indexOf(id) !== index), []);
  });
}
