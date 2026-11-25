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

test('a disabled editor gets no focus handlers; a value that is not a function is refused', () => {
  reset();
  const off = byId(render(() => h('page', {}, h('input', { value: '', disabled: true, onFocus() {}, 'aria-label': 'A', 'data-testid': 'a' }))), 'a');
  assert.equal(off.props.onFocus, undefined);
  assert.throws(() => prepare(() => h('page', {}, h('input', { value: '', onBlur: true, 'aria-label': 'A' })), { env: {} }), /UI_INPUT: onBlur takes a function/);
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

test('setSelection and replaceRange reach the bound editor as a counted range, after the value', () => {
  reset();
  let cell, changes = [];
  const view = () => {
    cell = useLocal('body', 'hello world');
    return h('page', {}, h('textarea', { bind: cell, 'aria-label': 'Body', 'data-testid': 'b', onSelectionChange: range => changes.push(range) }));
  };
  let body = byId(render(view), 'b');
  assert.equal(body.props.selectionRequest, undefined);
  cell.setSelection(6, 11);
  body = byId(render(view), 'b');
  assert.deepEqual(body.props.selectionRequest, [1, 6, 11]);
  cell.replaceRange(6, 11, 'there');
  body = byId(render(view), 'b');
  assert.equal(body.props.inputValue, 'hello there');
  assert.deepEqual(body.props.selectionRequest, [2, 11, 11], 'the caret after the new text');
  dispatch(body.props.onSelectionChange, JSON.stringify({ start: 2, end: 4 }));
  assert.deepEqual(changes, [{ start: 2, end: 4 }]);
});

test('a selection call checks its offsets against the value and keeps a surrogate pair whole', () => {
  reset();
  let cell;
  render(() => { cell = useLocal('t', 'a\u{1f600}b'); return h('page', {}, h('input', { bind: cell, 'aria-label': 'T' })); });
  assert.throws(() => cell.setSelection(2, 1), /UI_LOCAL: setSelection takes UTF-16 offsets/);
  assert.throws(() => cell.setSelection(0, 5), /0 <= start <= end <= 4/);
  assert.throws(() => cell.setSelection(2, 2), /split no character/);
  assert.throws(() => cell.replaceRange(0, 1, 7), /text a string/);
  cell.setSelection(1, 3);
});
