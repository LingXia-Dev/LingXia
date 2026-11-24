import { test } from 'node:test';
import assert from 'node:assert/strict';
import { h, List, prepare, commit, reset } from '../src/internal.js';
import { createView, syncData } from '../src/runtime/hooks.js';
import { tree } from './support/wire.mjs';

const row = i => ({ kind: 'visit', key: `r${i}`, title: `Row ${i}`, at: 1700000000 + i });
const appendOps = (from, count) => Array.from({ length: count }, (_, i) => ({ op: 'add', path: '/rows/-', value: row(from + i) }));

// QuickJS walks every element to answer Object.isFrozen and to freeze, so an append may touch the
// old rows only to copy them once; this counts the work an engine with a JIT would hide.
test('appending rows to a long List keys, checks and asks about only the new rows', () => {
  reset();
  let keyed = 0;
  const getKey = item => { keyed++; return item.key; };
  const renderItem = item => h('text', { style: { height: 44 } }, item.title);
  const page = view => h('page', {}, h(List, { items: view.data.rows, getKey, renderItem, getItemHeight: () => 44, style: { height: 800 } }));
  let revision = 1;
  prepare(page, createView({ rows: Array.from({ length: 5000 }, (_, i) => row(i)) }, {}, revision)); commit();
  const isFrozen = Object.isFrozen, longAsks = [];
  Object.isFrozen = value => { if (Array.isArray(value) && value.length > 1000) longAsks.push(value.length); return isFrozen(value); };
  try {
    for (let round = 0; round < 3; round++) {
      keyed = 0;
      assert.ok(syncData(revision, revision + 1, appendOps(5000 + round * 200, 200)));
      revision++;
      const records = prepare(page, createView(undefined, {})); commit();
      assert.equal(keyed, 200, 'only the appended rows are keyed');
      assert.equal(tree(records).children[0].props.listCount, 5200 + round * 200);
    }
    assert.deepEqual(longAsks, [], 'no long array is asked whether it is frozen');
  } finally { Object.isFrozen = isFrozen; }
});

test('an append keeps duplicate keys refused and the keys of a replaced row in place', () => {
  reset();
  const renderItem = item => h('text', { style: { height: 44 } }, item.title);
  const page = view => h('page', {}, h(List, { items: view.data.rows, getKey: item => item.key, renderItem, style: { height: 400 } }));
  let revision = 1;
  prepare(page, createView({ rows: [row(0), row(1)] }, {}, revision)); commit();
  assert.ok(syncData(revision, ++revision, appendOps(2, 2)));
  prepare(page, createView(undefined, {})); commit();
  // An in-place edit is not an extension: the keys are compared, not assumed.
  assert.ok(syncData(revision, ++revision, [{ op: 'replace', path: '/rows/0/title', value: 'First' }, { op: 'add', path: '/rows/-', value: row(4) }]));
  assert.ok(tree(prepare(page, createView(undefined, {}))).children[0].props.listCount === 5); commit();
  assert.ok(syncData(revision, ++revision, appendOps(1, 1)));
  assert.throws(() => prepare(page, createView(undefined, {})), /UI_LIST/);
});

test('an array frozen outside the runtime still counts as frozen', async () => {
  const { isFrozen } = await import('../src/runtime/data.js');
  const later = [row(0)];
  assert.equal(isFrozen(later), false);
  Object.freeze(later);
  assert.equal(isFrozen(later), true, 'freezing after an earlier ask is seen');
  assert.equal(isFrozen(Object.preventExtensions([row(1)])), false, 'closed but writable is not frozen');
  assert.equal(isFrozen(Object.seal([row(2)])), false, 'sealed but writable is not frozen');

  // A List over an array the page froze itself reuses its rows and draws no warning.
  reset();
  const warnings = [];
  globalThis.__lingxiaWarn = message => warnings.push(message);
  try {
    let rendered = 0;
    const items = Object.freeze(Array.from({ length: 3 }, (_, i) => row(i)));
    const renderItem = item => { rendered++; return h('text', { style: { height: 44 } }, item.title); };
    const page = () => h('page', {}, h(List, { items, getKey: item => item.key, renderItem, style: { height: 400 } }));
    prepare(page, createView({}, {}, 1)); commit();
    const first = rendered;
    assert.ok(first > 0);
    prepare(page, createView({}, {}, 2)); commit();
    assert.equal(rendered, first, 'rows are reused');
    assert.deepEqual(warnings, []);
  } finally { delete globalThis.__lingxiaWarn; }
});
