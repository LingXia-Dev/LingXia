import { test } from 'node:test';
import assert from 'node:assert/strict';
import { h, prepare, commit, dispatch, reset, useLocal } from '../src/internal.js';
import { decode } from './support/wire.mjs';

const render = view => { const out = decode(prepare(view, { env: {} })); commit(); return out; };
const byId = (nodes, id) => nodes.find(node => node.testId === id);

test('onFocus and onBlur are handlers on an input and a textarea; an editor without them sends neither', () => {
  reset();
  const seen = [];
  const view = () => {
    const a = useLocal('a', ''), b = useLocal('b', '');
    return h('page', {},
      h('input', { bind: a, 'aria-label': 'A', 'data-testid': 'a', onFocus: () => seen.push('a+'), onBlur: () => seen.push('a-') }),
      h('textarea', { bind: b, 'aria-label': 'B', 'data-testid': 'b', onFocus: () => seen.push('b+') }),
      h('input', { value: '', 'aria-label': 'C', 'data-testid': 'c' }));
  };
  const nodes = render(view);
  dispatch(byId(nodes, 'a').props.onFocus);
  dispatch(byId(nodes, 'a').props.onBlur);
  dispatch(byId(nodes, 'b').props.onFocus);
  assert.deepEqual(seen, ['a+', 'a-', 'b+']);
  assert.equal(byId(nodes, 'b').props.onBlur, undefined);
  assert.equal(byId(nodes, 'c').props.onFocus, undefined);
});

test('a disabled editor and an older host get no focus handlers; a value that is not a function is refused', () => {
  reset();
  const off = byId(render(() => h('page', {}, h('input', { value: '', disabled: true, onFocus() {}, 'aria-label': 'A', 'data-testid': 'a' }))), 'a');
  assert.equal(off.props.onFocus, undefined);
  assert.throws(() => prepare(() => h('page', {}, h('input', { value: '', onBlur: true, 'aria-label': 'A' })), { env: {} }), /UI_INPUT: onBlur takes a function/);
  const previous = globalThis.__lingxiaViewHost;
  globalThis.__lingxiaViewHost = { maxField: 246 };
  try {
    const old = byId(render(() => h('page', {}, h('input', { value: '', onFocus() {}, 'aria-label': 'A', 'data-testid': 'a' }))), 'a');
    assert.equal(old.props.onFocus, undefined);
  } finally { globalThis.__lingxiaViewHost = previous; }
});

test('readOnly travels as a flag only when true, and takes a boolean', () => {
  reset();
  const nodes = render(() => h('page', {},
    h('input', { value: 'kept', readOnly: true, 'aria-label': 'A', 'data-testid': 'a' }),
    h('textarea', { value: 'kept', readOnly: false, 'aria-label': 'B', 'data-testid': 'b' })));
  assert.equal(byId(nodes, 'a').props.readOnly, 1);
  assert.equal(byId(nodes, 'b').props.readOnly, undefined);
  assert.throws(() => prepare(() => h('page', {}, h('input', { value: '', readOnly: 'yes', 'aria-label': 'A' })), { env: {} }), /UI_INPUT: readOnly must be boolean/);
});
