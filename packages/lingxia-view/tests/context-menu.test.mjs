import { test } from 'node:test';
import assert from 'node:assert/strict';
import { h, prepare, commit, dispatch, reset } from '../src/internal.js';
import { decode } from './support/wire.mjs';

const render = view => { const out = decode(prepare(view, { env: {} })); commit(); return out; };
const byId = (nodes, id) => nodes.find(node => node.testId === id);

test('onContextMenu is a handler that receives the point and the pointer', () => {
  reset();
  const seen = [];
  const nodes = render(() => h('page', {},
    h('view', { onContextMenu: event => seen.push(event), 'data-testid': 'row' }, h('text', {}, 'Row')),
    h('view', { 'data-testid': 'plain' }, h('text', {}, 'Plain'))));
  dispatch(byId(nodes, 'row').props.onContextMenu, JSON.stringify({ x: 20, y: 15, pointer: 'touch' }));
  assert.deepEqual(seen, [{ x: 20, y: 15, pointer: 'touch' }]);
  assert.ok(Object.isFrozen(seen[0]));
  assert.equal(byId(nodes, 'plain').props.onContextMenu, undefined);
});

test('onContextMenu takes a function and belongs to a view, button, link or text', () => {
  reset();
  assert.throws(() => render(() => h('page', {}, h('view', { onContextMenu: 'menu' }))), /onContextMenu takes a function/);
  assert.throws(() => render(() => h('page', {}, h('image', { src: 'a.png', alt: 'A', onContextMenu() {} }))), /onContextMenu/);
});
