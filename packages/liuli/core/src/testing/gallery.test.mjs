import { test } from 'node:test';
import assert from 'node:assert/strict';
import { existsSync, readFileSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { render } from '@lingxia/view/testing';

const gallery = resolve(dirname(fileURLToPath(import.meta.url)), '../../../gallery');
const pages = JSON.parse(readFileSync(resolve(gallery, 'lxapp.json'), 'utf8')).pages;

// Device tests find a gallery page's elements by test id, so ids must not repeat across its examples.
for (const page of pages) {
  test(`gallery ${page.name}: every test id on the page is unique`, async () => {
    const screen = await render(resolve(gallery, `${page.path}.tsx`), {}, { root: gallery });
    const ids = screen.queryAll(node => Boolean(node.testId)).map(node => node.testId);
    const repeated = ids.filter((id, index) => ids.indexOf(id) !== index);
    assert.deepEqual(repeated, []);
  });
}

const tilesOf = async path => {
  const screen = await render(resolve(gallery, path), {}, { root: gallery });
  return screen.queryAll(node => node.testId?.startsWith('open-')).map(node => node.testId.slice('open-'.length));
};
const categoryPages = pages.map(page => page.name).filter(name => name.startsWith('category-'));
const leafPages = pages.map(page => page.name).filter(name => name !== 'home' && !name.startsWith('category-'));

// Home is an index: one card per category page, in manifest order.
test('gallery home: one card per category, in manifest order', async () => {
  assert.deepEqual(await tilesOf('pages/home/index.tsx'), categoryPages);
});

// Every other page is reached from exactly one category page, and the categories read in manifest order.
test('gallery categories: every page sits in exactly one category', async () => {
  const reached = [];
  for (const category of categoryPages) reached.push(...await tilesOf(`pages/${category}/index.tsx`));
  assert.deepEqual(reached, leafPages);
});

// Every component in the catalog is listed once in the generated index the home page searches.
test('gallery index: every catalog component appears exactly once', () => {
  const catalog = JSON.parse(readFileSync(resolve(gallery, '../core/catalog.json'), 'utf8')).components.map(entry => entry.name);
  const index = readFileSync(resolve(gallery, 'shared/catalog.generated.ts'), 'utf8');
  const listed = [...index.matchAll(/"name": "([^"]+)"/g)].map(match => match[1]);
  for (const name of catalog) assert.equal(listed.filter(each => each === name).length, 1, name);
});

// The device spec walks home → category → page from its own lists; keep them equal to the gallery.
// The device specs are local and untracked, so this is skipped where they are absent.
const deviceSpec = resolve(gallery, 'zz-tests/gallery.test.ts');
test('gallery device spec: walks every category and page', { skip: !existsSync(deviceSpec) }, async () => {
  const spec = readFileSync(deviceSpec, 'utf8');
  const block = spec.match(/const categories: Record<string, string\[\]> = \{([\s\S]*?)\n\};/)[1];
  const walked = Object.fromEntries([...block.matchAll(/'([a-z-]+)': \[([^\]]*)\]/g)].map(([, category, list]) => [category, [...list.matchAll(/'([a-z-]+)'/g)].map(match => match[1])]));
  const expected = {};
  for (const category of categoryPages) expected[category] = await tilesOf(`pages/${category}/index.tsx`);
  assert.deepEqual(walked, expected);
});

test('gallery home: search lists matches by name, summary or category, and says when nothing matches', async () => {
  const screen = await render(resolve(gallery, 'pages/home/index.tsx'), {}, { root: gallery });
  screen.type(screen.getByTestId('home-search'), 'date');
  const results = () => screen.queryAll(node => node.testId?.startsWith('result-')).map(node => node.testId.slice('result-'.length));
  assert.ok(results().includes('date-input'));
  assert.equal(screen.queryAll(node => node.testId?.startsWith('open-category-')).length, 0);
  screen.type(screen.getByTestId('home-search'), 'overlays');
  assert.deepEqual(results(), ['dialog', 'dropdown-menu', 'tooltip', 'bottom-sheet', 'popover', 'drawer', 'hover-card', 'context-menu', 'command-palette', 'tour']);
  screen.type(screen.getByTestId('home-search'), 'zzzz');
  assert.equal(results().length, 0);
  screen.getByTestId('home-no-results');
});

test('gallery home: a table of contents in three groups, each row with its icon, line and count; nothing is dated', async () => {
  const screen = await render(resolve(gallery, 'pages/home/index.tsx'), {}, { root: gallery });
  const lists = screen.queryAll(node => node.props.role === 'list');
  assert.deepEqual(lists.map(list => list.props.accessibilityLabel), ['Foundations', 'Components', 'Patterns']);
  assert.deepEqual(lists.map(list => list.children.length), [1, categoryPages.length - 2, 1]);
  const row = screen.getByTestId('open-category-inputs');
  assert.equal(row.tag, 'button');
  const [icon, text, count, chevron] = row.children;
  assert.deepEqual([icon.props.width, icon.props.background], [32, 'accent-soft'], 'one tint for every row');
  assert.equal(text.children[1].text, 'Text, dates and amounts', 'a line short enough to stay on one line of a phone');
  assert.match(count.children[0].text, /^\d+$/);
  assert.equal(chevron.tag, 'vector');
  assert.equal(screen.queryAll(node => /^new-/.test(node.testId ?? '') || node.text === 'New').length, 0);
  const catalog = readFileSync(resolve(gallery, 'shared/catalog.generated.ts'), 'utf8');
  assert.equal(/export const newest|"(since|status)":/.test(catalog), false, 'the catalog carries no dates or statuses');
});
