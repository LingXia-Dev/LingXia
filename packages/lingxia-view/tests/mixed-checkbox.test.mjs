import { test } from 'node:test';
import assert from 'node:assert/strict';
import { h, prepare, commit, dispatch, reset } from '../src/internal.js';
import { compileUtilities } from '../compiler/tailwind.mjs';
import { decode } from './support/wire.mjs';

const render = view => { const out = decode(prepare(view, { env: {} })); commit(); return out; };
const byId = (nodes, id) => nodes.find(node => node.testId === id);

test('an indeterminate checkbox is mixed on the wire and a press reports true', () => {
  reset();
  const seen = [];
  const nodes = render(() => h('page', {},
    h('checkbox', { value: false, indeterminate: true, onChange: next => seen.push(next), 'aria-label': 'All', 'data-testid': 'all' }),
    h('checkbox', { value: true, indeterminate: true, onChange: next => seen.push(next), 'aria-label': 'Some', 'data-testid': 'some' }),
    h('checkbox', { value: true, indeterminate: false, onChange: next => seen.push(next), 'aria-label': 'One', 'data-testid': 'one' })));
  assert.equal(byId(nodes, 'all').props.mixed, 1);
  assert.equal(byId(nodes, 'one').props.mixed, undefined);
  for (const id of ['all', 'some', 'one']) dispatch(byId(nodes, id).props.onCheckedChange);
  assert.deepEqual(seen, [true, true, false]);
});

test('indeterminate belongs to a checkbox and aria-checked="mixed" to a menuitemcheckbox', () => {
  reset();
  assert.throws(() => render(() => h('page', {}, h('switch', { value: false, indeterminate: true, onChange() {}, 'aria-label': 'S' }))), /indeterminate is not a property of <switch>/);
  assert.throws(() => render(() => h('page', {}, h('checkbox', { value: false, indeterminate: 'yes', onChange() {}, 'aria-label': 'C' }))), /UI_CHECK: indeterminate/);
  reset();
  const item = byId(render(() => h('page', {}, h('button', { role: 'menuitemcheckbox', 'aria-checked': 'mixed', onClick() {}, 'data-testid': 'item' }, 'Columns'))), 'item');
  assert.equal(item.props.mixed, 1);
  assert.equal(item.props.stateChecked, 0);
  reset();
  assert.throws(() => render(() => h('page', {}, h('button', { role: 'radio', 'aria-checked': 'mixed', onClick() {} }, 'A'))), /aria-checked="mixed"/);
});

test('a disabled link lowers as a link that is not enabled and has no handler', () => {
  reset();
  const link = byId(render(() => h('page', {}, h('text', {}, 'Read ', h('link', { disabled: true, onClick() {}, 'data-testid': 'off' }, 'terms')))), 'off');
  assert.equal(link.props.enabled, 0);
  assert.equal(link.props.onClick, undefined);
});

test('indeterminate: compiles to a rule that follows the checkbox', async () => {
  const rules = (await compileUtilities(new Set(['indeterminate:bg-accent', 'bg-surface']))).filter(rule => rule.name === 'indeterminate:bg-accent');
  assert.equal(rules.length, 1);
  assert.equal(rules[0].error, undefined);
  assert.equal(rules[0].when.indeterminate, true);
});
