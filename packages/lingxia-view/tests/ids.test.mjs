import { test, beforeEach } from 'node:test';
import assert from 'node:assert/strict';
import { decode, rendered } from './support/wire.mjs';
import { h, listTag, describe, prepare, commit, reset, useLocal, useId, createView } from '../src/internal.js';
import { stampIds } from '../compiler/index.mjs';

beforeEach(() => reset());

const all = { root: { start: 0, end: 3, leading: 0, trailing: 0 } };
const ids = buffer => decode(buffer).filter(node => node.props.elementId).map(node => node.props.elementId);

test('every element takes an id; a run does not, and ids hold no spaces', () => {
  for (const tag of ['view', 'text', 'image', 'scroll'])
    assert.equal(rendered(() => h('page', {}, h(tag, { id: `the-${tag}`, ...(tag === 'image' ? { src: 'a.png' } : {}) }))).children[0].props.elementId, `the-${tag}`);
  assert.equal(rendered(() => h('page', {}, h('input', { id: 'field', value: '' }))).children[0].props.elementId, 'field');
  assert.throws(() => describe(() => h('page', {}, h('text', {}, h('span', { id: 'run' }, 'x'))), {}), /UI_PROP: <span> is a run of its <text>; put id on the <text>/);
  assert.throws(() => describe(() => h('page', {}, h('view', { id: 'a b' })), {}), /UI_ID: an id is 1\.\.256 characters without spaces/);
});

test('an open popover carries its id as the element id', () => {
  const page = () => h('page', {}, h('view', { id: 'menu', popover: 'manual', open: true, 'aria-label': 'Menu' }, h('text', {}, 'x')));
  assert.deepEqual(ids(describe(page, {})), ['menu']);
});

test('useId is stable across renders, distinct per instance, call site and list row', () => {
  function Field({ label }) { const a = useId('Field#0'), b = useId('Field#1'); return h('view', { id: a, 'aria-label': label }, h('text', { id: b }, label)); }
  let n;
  const page = () => { n = useLocal('n', 0);
    return h('page', {}, h(Field, { key: 'one', label: `one ${n.value}` }), h(Field, { key: 'two', label: 'two' }),
      h(listTag, { count: 2, itemKeys: ['p', 'q'], row: () => h(Field, { label: 'row' }) })); };
  const first = ids(prepare(page, createView({}, {}), all)); commit();
  assert.equal(new Set(first).size, first.length, 'all distinct');
  assert.ok(first.every(id => /^lx-[0-9a-z]+$/.test(id)));
  n.set(1);
  assert.deepEqual(ids(prepare(page, createView({}, {}), all)), first);
  assert.throws(() => useId('x'), /UI_SCOPE/);
});

test('uncompiled code keys useId by call order', () => {
  function Pair() { return h('view', { id: useId() }, h('text', { id: useId() }, 'x')); }
  const [a, b] = ids(describe(() => h('page', {}, h(Pair)), {}));
  assert.ok(a && b && a !== b);
});

test('the compiler stamps each useId call with its function and ordinal', () => {
  const out = stampIds("function Tabs(){ const a = useId(); const b = useId(); return a + b; }\nconst Panel = () => useId();\nuseId('kept');", 'tabs.tsx');
  assert.match(out, /useId\("Tabs#0"\).*useId\("Tabs#1"\)/s);
  assert.match(out, /useId\("Panel#0"\)/);
  assert.match(out, /useId\('kept'\)/);
});

test('useId stays stable and distinct past the id cache, across renders that evict it', () => {
  // More ids than the cache keeps (4096), so a second render re-derives evicted ones.
  const rows = 1100;
  function Row() { return h('text', {}, [useId('Row#0'), useId('Row#1'), useId('Row#2'), useId('Row#3')].join(' ')); }
  const page = () => h('page', {}, Array.from({ length: rows }, (_, i) => h(Row, { key: `r${i}` })));
  const every = { root: { start: 0, end: 1, leading: 0, trailing: 0 } };
  const texts = buffer => decode(buffer).filter(node => node.tag === 'text').flatMap(node => node.text.split(' '));
  const first = texts(prepare(page, createView({}, {}), every)); commit();
  assert.equal(first.length, 4 * rows);
  assert.equal(new Set(first).size, 4 * rows, 'all distinct');
  assert.deepEqual(texts(prepare(page, createView({}, {}), every)), first);
});
