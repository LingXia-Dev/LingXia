import { test } from 'node:test';
import assert from 'node:assert/strict';
import { stableCallbacks } from './stable.ts';
import { selectCost } from './select.ts';
import { callout, codeBlock, heading, list, listItem, paragraph } from '../schema/index.ts';

test('stableCallbacks: the same functions on every render, each calling the latest one', () => {
  const anchor = {};
  const calls = [];
  const first = stableCallbacks(anchor, { onA: x => calls.push(['first', x]), onB: undefined });
  const second = stableCallbacks(anchor, { onA: x => calls.push(['second', x]), onB: () => calls.push(['b']) });
  assert.equal(first.onA, second.onA, 'a block keeps its last render while only the closures changed');
  assert.ok(!('onB' in first), 'an absent callback stays absent');
  first.onA(1);
  second.onB();
  assert.deepEqual(calls, [['second', 1], ['b']]);
  assert.notEqual(stableCallbacks({}, { onA: () => {} }).onA, first.onA, 'each editor has its own');
});

test('selectCost: one per paragraph, heading or code block, nested ones included', () => {
  assert.equal(selectCost(paragraph('a')), 1);
  assert.equal(selectCost(heading(2, 'h')), 1);
  assert.equal(selectCost(codeBlock('ts', 'x')), 1);
  assert.equal(selectCost(list('bullet', [listItem('a'), listItem('b')])), 0);
  assert.equal(selectCost(callout('info', [paragraph('a'), paragraph('b')])), 2);
});
