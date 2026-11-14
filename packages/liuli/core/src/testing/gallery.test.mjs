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
  assert.deepEqual(results(), ['dialog', 'dropdown-menu', 'tooltip', 'bottom-sheet', 'popover']);
  screen.type(screen.getByTestId('home-search'), 'zzzz');
  assert.equal(results().length, 0);
  screen.getByTestId('home-no-results');
});

// A home preview shows real components, not drawings of them: some JSX tag in it resolves, through
// specimen functions, to a component of the package. Foundations draws tokens, which are not
// components, and media is a still frame so the home page loads no video.
const drawn = new Set(['FoundationsPreview', 'MediaPreview']);
const packageSources = resolve(gallery, '../core/src');
function usesComponent(file, name, seen = new Set()) {
  if (seen.has(`${file}#${name}`)) return false;
  seen.add(`${file}#${name}`);
  const source = readFileSync(file, 'utf8');
  const start = source.indexOf(`export function ${name}(`);
  if (start < 0) return false;
  const next = source.indexOf('\nexport function ', start + 1);
  const body = source.slice(start, next < 0 ? undefined : next);
  const imports = new Map();
  for (const [, names, from] of source.matchAll(/import \{([^}]+)\} from '([^']+)'/g))
    for (const part of names.split(','))
      if (part.trim()) {
        const [imported, local = imported] = part.trim().split(/\s+as\s+/);
        imports.set(local, { imported, from });
      }
  for (const [, tag] of body.matchAll(/<([A-Z][A-Za-z]*)/g)) {
    const origin = imports.get(tag);
    if (origin?.from.startsWith('@liulidesign/')) return true;
    // Inside the package's own sources (an examples file imports its siblings relatively).
    if (origin?.from.startsWith('.') && !origin.from.includes('.examples') && resolve(dirname(file), origin.from).startsWith(packageSources)) return true;
    const local = origin ? resolve(dirname(file), `${origin.from}.tsx`) : file;
    if (existsSync(local) && usesComponent(local, origin?.imported ?? tag, seen)) return true;
  }
  return false;
}

test('gallery home: every preview shows at least one real component', () => {
  const file = resolve(gallery, 'shared/Previews.tsx');
  const names = [...readFileSync(file, 'utf8').matchAll(/export function ([A-Za-z]+Preview)\(/g)].map(match => match[1]);
  assert.ok(names.length >= 10);
  const drawings = names.filter(name => !drawn.has(name) && !usesComponent(file, name));
  assert.deepEqual(drawings, []);
});
