import { test } from 'node:test';
import assert from 'node:assert/strict';
import { h, prepare, commit, dispatch, reset } from '../src/internal.js';
import { decode } from './support/wire.mjs';

const render = view => { const out = decode(prepare(view, { env: {} })); commit(); return out; };
const byId = (nodes, id) => nodes.find(node => node.testId === id);

test('onKeyDown sends its keys as the capture set and calls the handler of the key delivered', () => {
  reset();
  const seen = [];
  const nodes = render(() => h('page', {},
    h('input', { value: '', 'aria-label': 'Search', 'data-testid': 'q',
      onKeyDown: { ArrowDown: () => seen.push('down'), Enter: () => seen.push('enter'), Home: () => seen.push('home') } }),
    h('input', { value: '', 'aria-label': 'Plain', 'data-testid': 'plain' })));
  const q = byId(nodes, 'q');
  assert.equal(q.props.keyMap, 'ArrowDown Enter Home');
  dispatch(q.props.onKeyDown, 'Enter');
  dispatch(q.props.onKeyDown, 'ArrowDown');
  dispatch(q.props.onKeyDown, 'ArrowUp');
  assert.deepEqual(seen, ['enter', 'down'], 'a key the map does not list calls nothing');
  assert.equal(byId(nodes, 'plain').props.keyMap, undefined);
  assert.equal(byId(nodes, 'plain').props.onKeyDown, undefined);
});

test('onKeyDown takes only the eight keys mapped to functions; a disabled editor gets no map', () => {
  reset();
  const page = props => () => prepare(() => h('page', {}, h('input', { value: '', 'aria-label': 'A', ...props })), { env: {} });
  assert.throws(page({ onKeyDown: { Tab() {} } }), /UI_INPUT: onKeyDown maps ArrowUp, ArrowDown, Enter, Escape, Home, End, PageUp, PageDown to functions/);
  assert.throws(page({ onKeyDown: { Enter: true } }), /UI_INPUT: onKeyDown maps/);
  assert.throws(page({ onKeyDown: {} }), /UI_INPUT: onKeyDown maps/);
  assert.throws(page({ onKeyDown() {} }), /UI_INPUT: onKeyDown maps/);
  const off = byId(render(() => h('page', {}, h('input', { value: '', disabled: true, 'aria-label': 'A', 'data-testid': 'a', onKeyDown: { Enter() {} } }))), 'a');
  assert.equal(off.props.keyMap, undefined);
});
