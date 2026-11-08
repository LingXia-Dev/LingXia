import { test } from 'node:test';
import assert from 'node:assert/strict';
import { applyPatch } from '../src/runtime/data.js';
import { createView, syncData } from '../src/runtime/hooks.js';

// The native tree arrives with its keys in code point order.
const sorted = value => value === null || typeof value !== 'object' ? value : Array.isArray(value) ? value.map(sorted)
  : Object.fromEntries(Object.keys(value).sort().map(key => [key, sorted(value[key])]));
const frozenDeep = value => value === null || typeof value !== 'object'
  || (Object.isFrozen(value) && Object.values(value).every(frozenDeep));

test('a replay matches the native replica, op by op', () => {
  const base = { list: [{ n: 1 }, { n: 2 }, { n: 3 }], meta: { title: 'a' } };
  const cases = [
    [[{ op: 'replace', path: '/list/1/n', value: 20 }], { list: [{ n: 1 }, { n: 20 }, { n: 3 }], meta: { title: 'a' } }],
    [[{ op: 'add', path: '/list/1', value: { n: 9 } }], { list: [{ n: 1 }, { n: 9 }, { n: 2 }, { n: 3 }], meta: { title: 'a' } }],
    [[{ op: 'add', path: '/list/-', value: { n: 4 } }], { list: [{ n: 1 }, { n: 2 }, { n: 3 }, { n: 4 }], meta: { title: 'a' } }],
    [[{ op: 'add', path: '/list/99', value: 0 }], { list: [{ n: 1 }, { n: 2 }, { n: 3 }, 0], meta: { title: 'a' } }],
    [[{ op: 'replace', path: '/list/5', value: 6 }], { list: [{ n: 1 }, { n: 2 }, { n: 3 }, null, null, 6], meta: { title: 'a' } }],
    [[{ op: 'remove', path: '/list/0' }, { op: 'remove', path: '/list/7' }], { list: [{ n: 2 }, { n: 3 }], meta: { title: 'a' } }],
    [[{ op: 'remove', path: '/meta' }, { op: 'remove', path: '/absent' }], { list: [{ n: 1 }, { n: 2 }, { n: 3 }] }],
    [[{ op: 'add', path: '/deep/items/x', value: 1 }], { list: [{ n: 1 }, { n: 2 }, { n: 3 }], meta: { title: 'a' }, deep: { items: { x: 1 } } }],
    [[{ op: 'add', path: '/rows/0', value: 'r' }], { list: [{ n: 1 }, { n: 2 }, { n: 3 }], meta: { title: 'a' }, rows: ['r'] }],
    [[{ op: 'add', path: '/fresh/0', value: 'v' }, { op: 'add', path: '/fresh/1', value: 'w' }], { list: [{ n: 1 }, { n: 2 }, { n: 3 }], meta: { title: 'a' }, fresh: ['v', 'w'] }],
    [[{ op: 'replace', path: '/a~1b/~0c', value: 1 }], { list: [{ n: 1 }, { n: 2 }, { n: 3 }], meta: { title: 'a' }, 'a/b': { '~c': 1 } }],
    [[{ op: 'replace', path: '', value: { only: true } }], { only: true }],
    [[{ op: 'remove', path: '' }, { op: 'add', path: '/k', value: 1 }], { k: 1 }],
  ];
  for (const [ops, expected] of cases) {
    const result = applyPatch(applyPatch(Object.freeze({}), [{ op: 'replace', path: '', value: base }]), ops);
    assert.deepEqual(result, expected, JSON.stringify(ops));
    assert.equal(JSON.stringify(result), JSON.stringify(sorted(expected)), `key order: ${JSON.stringify(ops)}`);
    assert.ok(frozenDeep(result), JSON.stringify(ops));
  }
});

test('untouched parts keep their identity and an equal write changes nothing', () => {
  const base = applyPatch(Object.freeze({}), [{ op: 'replace', path: '', value: { rows: [{ n: 1 }, { n: 2 }], other: { x: 1 } } }]);
  const next = applyPatch(base, [{ op: 'replace', path: '/rows/1/n', value: 5 }]);
  assert.equal(next.other, base.other);
  assert.equal(next.rows[0], base.rows[0]);
  assert.notEqual(next.rows, base.rows);
  assert.equal(applyPatch(base, [{ op: 'replace', path: '/other', value: { x: 1 } }]), base);
});

test('a `__proto__` key stays data', () => {
  const result = applyPatch(Object.freeze({}), [{ op: 'add', path: '/__proto__', value: { polluted: true } }]);
  assert.equal(Object.getPrototypeOf(result), Object.prototype);
  assert.deepEqual(Object.keys(result), ['__proto__']);
  assert.equal({}.polluted, undefined);
});

test('the isolate replays only from the revision it holds', () => {
  assert.equal(syncData(0, 1, []), false, 'no whole tree yet');
  createView({ n: 1, keep: { a: 1 } }, {}, 3);
  const before = createView(undefined, {}).data;
  assert.equal(syncData(2, 4, [{ op: 'replace', path: '/n', value: 2 }]), false, 'stale base');
  assert.equal(syncData(3, 4, [{ op: 'replace', path: '/n', value: 2 }]), true);
  const after = createView(undefined, {}).data;
  assert.deepEqual(after, { n: 2, keep: { a: 1 } });
  assert.equal(after.keep, before.keep);
  assert.equal(syncData(4, 4, []), true, 'no writes since the last frame');
  createView({ n: 9 }, {});
  assert.equal(syncData(4, 5, []), false, 'a tree without a revision is not replayable');
});
