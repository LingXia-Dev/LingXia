import { test } from 'node:test';
import assert from 'node:assert/strict';
import { textOf } from '@lingxia/view/testing';
import { described, renderExample } from '../testing/harness.mjs';

test('Bound: a textarea named by its label, described by its help, growing from rows to maxRows', async () => {
  const screen = await renderExample('TextArea/TextArea', 'Bound');
  const note = () => screen.getByTestId('note');
  assert.equal(note().tag, 'textarea');
  assert.equal(note().props.accessibilityLabel, 'Delivery note');
  assert.equal(note().props.placeholder, 'Gate code, where to leave it');
  assert.equal(note().props.rows, 3);
  assert.equal(note().props.maxRows, 6);
  assert.equal(note().props.maxLength, 280);
  assert.deepEqual(described(screen, note()), ['The courier sees this when they arrive.']);
  screen.type(note(), 'Blue door.\nRing twice.');
  assert.equal(note().props.inputValue, 'Blue door.\nRing twice.');
});

test('Reply: the send key runs onSubmit with the text, and the app clears it', async () => {
  const screen = await renderExample('TextArea/TextArea', 'Reply');
  const reply = () => screen.getByTestId('reply');
  assert.equal(reply().props.enterKeyHint, 'send');
  assert.equal(reply().props.rows, 1);
  screen.type(reply(), 'On my way');
  screen.submit(reply());
  assert.equal(textOf(screen.getByTestId('sent')), 'On my way');
  assert.equal(reply().props.inputValue, '');
});

test('Invalid: required and invalid until the text is long enough', async () => {
  const screen = await renderExample('TextArea/TextArea', 'Invalid');
  const reason = () => screen.getByTestId('reason');
  assert.equal(reason().props.stateRequired, 1);
  assert.equal(reason().props.stateInvalid, 1);
  assert.deepEqual(described(screen, reason()), ['Write at least 10 characters.']);
  screen.type(reason(), 'Moving abroad');
  assert.equal(reason().props.stateInvalid, undefined);
});

test('the editor keeps its own inset and matches TextInput text at each size', async () => {
  const screen = await renderExample('TextArea/TextArea', 'Sizes');
  const editors = screen.queryAll(node => node.tag === 'textarea');
  assert.equal(editors.length, 3);
  assert.deepEqual(editors.map(node => node.props.paddingTop), [8, 8, 8]);
  assert.equal(editors[0].props.fontSize < editors[1].props.fontSize, true);
  assert.equal(editors[1].props.fontSize < editors[2].props.fontSize, true);
});

test('maxRows defaults to at least rows', async () => {
  const screen = await renderExample('TextArea/TextArea', 'StateEmpty');
  const [editor] = screen.queryAll(node => node.tag === 'textarea');
  assert.equal(editor.props.rows, 2);
  assert.equal(editor.props.maxRows, 8);
});
