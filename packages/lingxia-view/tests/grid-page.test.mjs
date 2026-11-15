import { test } from 'node:test';
import assert from 'node:assert/strict';
import { h, prepare, commit, dispatch, reset } from '../src/internal.js';
import { decode } from './support/wire.mjs';

const render = view => { const out = decode(prepare(view, { env: {} })); commit(); return out; };
const byId = (nodes, id) => nodes.find(node => node.testId === id);

test('onPage on a grid hears previous and next', () => {
  reset();
  const seen = [];
  const nodes = render(() => h('page', {}, h('view', { role: 'grid', 'aria-label': 'Days', onPage: direction => seen.push(direction), 'data-testid': 'days' },
    h('view', { role: 'row' }, h('button', { role: 'gridcell', onClick() {} }, '1')))));
  dispatch(byId(nodes, 'days').props.onPage, 'next');
  dispatch(byId(nodes, 'days').props.onPage, 'previous');
  assert.deepEqual(seen, ['next', 'previous']);
});

test('onPage belongs to a grid and takes a function', () => {
  reset();
  assert.throws(() => render(() => h('page', {}, h('view', { onPage() {} }))), /onPage belongs to a <view role="grid">/);
  assert.throws(() => render(() => h('page', {}, h('view', { role: 'grid', 'aria-label': 'Days', onPage: 'next' }))), /onPage takes a function/);
});
