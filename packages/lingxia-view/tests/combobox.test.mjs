import { test } from 'node:test';
import assert from 'node:assert/strict';
import { h, prepare, commit, reset } from '../src/internal.js';
import { decode } from './support/wire.mjs';

const render = view => { const out = decode(prepare(view, { env: {} })); commit(); return out; };
const byId = (nodes, id) => nodes.find(node => node.testId === id);
const refuses = (node, pattern) => assert.throws(() => prepare(() => h('page', {}, node()), { env: {} }), pattern);

test('a combobox input names its active option; a combobox button says what is chosen', () => {
  reset();
  const nodes = render(() => h('page', {},
    h('input', { value: '', role: 'combobox', 'aria-expanded': true, 'aria-controls': 'list', 'aria-autocomplete': 'list',
      'aria-activedescendant': 'opt-1', 'aria-label': 'Fruit', 'data-testid': 'q' }),
    h('button', { role: 'combobox', 'aria-expanded': false, 'aria-controls': 'list', 'aria-valuetext': 'Kenya', 'aria-label': 'Country', 'data-testid': 'c', onClick() {} }, 'Country')));
  assert.equal(byId(nodes, 'q').props.role, 'combobox');
  assert.equal(byId(nodes, 'q').props.ariaActiveDescendant, 'opt-1');
  assert.equal(byId(nodes, 'c').props.role, 'combobox');
  assert.equal(byId(nodes, 'c').props.ariaValueText, 'Kenya');
  assert.deepEqual([byId(nodes, 'q').props.ariaControls, byId(nodes, 'q').props.ariaAutocomplete], ['list', 'list'], 'both reach the wire');
  assert.deepEqual([byId(nodes, 'c').props.ariaControls, byId(nodes, 'c').props.ariaAutocomplete], ['list', undefined]);
  const many = byId(render(() => h('page', {}, h('input', { value: '', role: 'combobox', 'aria-controls': ' recent  results ', 'aria-label': 'Search', 'data-testid': 'm' }))), 'm');
  assert.equal(many.props.ariaControls, 'recent results');
});

test('a spinbutton is a number input with a value; inputMode may pick the number keyboard beside type="number"', () => {
  reset();
  const node = byId(render(() => h('page', {}, h('input', { type: 'number', inputMode: 'decimal', value: '71.5', role: 'spinbutton',
    'aria-valuenow': 71.5, 'aria-valuemin': 0, 'aria-valuemax': 300, 'aria-valuetext': '71.5 kg', 'aria-label': 'Weight', 'data-testid': 'w' }))), 'w');
  assert.equal(node.props.role, 'spinbutton');
  assert.equal(node.props.ariaValueNow, 71.5);
  assert.equal(node.props.ariaValueText, '71.5 kg');
  assert.equal(node.props.keyboard, 'number');
  refuses(() => h('input', { type: 'number', inputMode: 'email', value: '', 'aria-label': 'A' }), /UI_INPUT: type="number" picks its own keyboard/);
  refuses(() => h('input', { type: 'date', inputMode: 'numeric', value: '', 'aria-label': 'A' }), /UI_INPUT: type="date" picks its own keyboard/);
});

test('the new roles and their properties refuse the wrong element and the wrong values', () => {
  reset();
  refuses(() => h('view', { role: 'combobox' }), /UI_PROP: role="combobox" belongs on an <input> or a <button>/);
  refuses(() => h('button', { role: 'spinbutton', onClick() {} }, 'B'), /UI_PROP: role="spinbutton" belongs on an <input>/);
  refuses(() => h('button', { role: 'combobox', 'aria-valuenow': 2, onClick() {} }, 'B'), /UI_PROP: role="combobox" takes aria-valuetext only/);
  refuses(() => h('button', { 'aria-activedescendant': 'x', onClick() {} }, 'B'), /UI_PROP: aria-activedescendant belongs on an <input> or <textarea>/);
  refuses(() => h('input', { value: '', 'aria-label': 'A', 'aria-activedescendant': 'two words' }), /UI_ID: aria-activedescendant names an element id/);
  refuses(() => h('input', { value: '', 'aria-label': 'A', 'aria-controls': 'list' }), /UI_PROP: aria-controls and aria-autocomplete belong on role="combobox"/);
  refuses(() => h('input', { value: '', 'aria-label': 'A', role: 'combobox', 'aria-autocomplete': 'inline' }), /UI_PROP: aria-autocomplete is "list" or "none"/);
  refuses(() => h('input', { value: '', 'aria-label': 'A', role: 'combobox', 'aria-controls': ' ' }), /UI_ID: aria-controls lists 1..8 element ids/);
  refuses(() => h('input', { value: '', 'aria-label': 'A', role: 'combobox', 'aria-controls': 'a b c d e f g h i' }), /UI_ID: aria-controls lists 1..8 element ids/);
});
