import { test } from 'node:test';
import assert from 'node:assert/strict';
import { textOf } from '@lingxia/view/testing';
import { described, renderExample } from '../testing/harness.mjs';

const status = screen => screen.queryAll(node => node.props.role === 'status')[0];

test('Bound: named by its label, typing reaches the bound cell, and validation shows in the status region', async () => {
  const screen = await renderExample('TextInput/TextInput', 'Bound');
  const email = () => screen.getByTestId('email');
  assert.equal(email().props.accessibilityLabel, 'Email');
  assert.equal(email().props.placeholder, 'you@example.com');
  assert.equal(email().props.stateRequired, 1);
  assert.deepEqual(described(screen, email()), ['We send the receipt here.']);
  assert.equal(textOf(status(screen)), '');
  screen.type(email(), 'ada');
  assert.equal(email().props.inputValue, 'ada');
  assert.equal(email().props.stateInvalid, 1);
  assert.deepEqual(described(screen, email()), ['We send the receipt here.', 'Add the part after @, such as example.com.']);
  screen.type(email(), 'ada@example.com');
  assert.equal(email().props.stateInvalid, undefined);
  assert.equal(textOf(status(screen)), '');
});

test('Search: the clear button appears with text, is named by clearLabel, and empties the field', async () => {
  const screen = await renderExample('TextInput/TextInput', 'Search');
  const clear = () => screen.queryAll(node => node.tag === 'button' && node.props.accessibilityLabel === 'Clear search');
  assert.equal(clear().length, 0);
  assert.ok(!screen.queryAll(node => node.props.accessibilityHidden === 1 && node.tag === 'text').length, 'no visible label is drawn');
  screen.type(screen.getByTestId('search'), 'groceries');
  assert.equal(clear().length, 1);
  screen.press(clear()[0]);
  assert.equal(screen.getByTestId('search').props.inputValue, '');
  assert.equal(clear().length, 0);
});

test('Disabled: the editor takes no input and the reason replaces the help text', async () => {
  const screen = await renderExample('TextInput/TextInput', 'Disabled');
  const code = screen.getByTestId('code');
  assert.equal(code.props.onInput, undefined);
  assert.equal(code.props.inputValue, 'LX-2041');
  assert.deepEqual(described(screen, code), ['Codes are fixed once sent.']);
});
