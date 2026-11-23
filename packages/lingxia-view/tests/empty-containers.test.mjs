// A page data field changing between null and an empty container must reach the View's copy.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { applyPatch, shared } from '../src/runtime/data.js';

const cases = [
  [null, []], [null, {}], [[], null], [undefined, []], [[], [1]], [{}, null],
  [{}, []], [[], {}], ['x', []], [0, []], [false, {}],
];

test('a whole value replaces a different kind, empty or not', () => {
  for (const [before, after] of cases) {
    const got = shared(after, before);
    assert.deepEqual(got, after, `${JSON.stringify(before)} -> ${JSON.stringify(after)}`);
  }
});

test('a replayed replace op lands an empty container over null, top-level and nested', () => {
  for (const [before, after] of cases) {
    for (const [root, path, read] of [
      // `undefined` is an absent key, as JSON carries it; a list holds no absent item.
      [before === undefined ? {} : { boosted: before }, '/boosted', (d) => d.boosted],
      [{ outer: before === undefined ? {} : { boosted: before } }, '/outer/boosted', (d) => d.outer.boosted],
      ...(before === undefined ? [] : [[{ list: [before] }, '/list/0', (d) => d.list[0]]]),
    ]) {
      const frozen = shared(root, undefined);
      const next = applyPatch(frozen, [{ op: 'replace', path, value: after }]);
      assert.deepEqual(read(next), after, `${path}: ${JSON.stringify(before)} -> ${JSON.stringify(after)}`);
    }
  }
});
