import { test } from 'node:test';
import assert from 'node:assert/strict';
import { textOf } from '@lingxia/view/testing';
import { renderExample } from '../testing/harness.mjs';

test('Filters: each toggle is a named button reporting pressed; a press flips it and the list', async () => {
  const screen = await renderExample('ToggleButton/ToggleButton', 'Filters');
  const unread = () => screen.getByTestId('unread');
  assert.equal(unread().tag, 'button');
  assert.equal(unread().props.accessibilityLabel, 'Unread');
  assert.equal(unread().props.statePressed, 0);
  assert.equal(textOf(screen.getByTestId('count')), '4 notes');
  screen.press(unread());
  assert.equal(unread().props.statePressed, 1);
  assert.equal(unread().props.background, 'accent-soft');
  assert.equal(textOf(screen.getByTestId('count')), '2 notes');
  screen.press(screen.getByTestId('starred'));
  assert.equal(textOf(screen.getByTestId('count')), '1 note');
  screen.press(unread());
  assert.equal(unread().props.statePressed, 0);
  assert.equal(unread().props.accessibilityLabel, 'Unread', 'the name does not change with the state');
});

test('StateDisabledOn: disabled keeps the pressed state but takes no press', async () => {
  const screen = await renderExample('ToggleButton/ToggleButton', 'StateDisabledOn');
  const button = screen.queryAll(node => node.tag === 'button')[0];
  assert.equal(button.props.statePressed, 1);
  assert.equal(button.props.onClick, undefined);
});

test('Sizes: heights come from the control tokens', async () => {
  const screen = await renderExample('ToggleButton/ToggleButton', 'Sizes');
  assert.deepEqual(screen.queryAll(node => node.tag === 'button').map(node => node.props.minHeight), [32, 40, 48]);
});
