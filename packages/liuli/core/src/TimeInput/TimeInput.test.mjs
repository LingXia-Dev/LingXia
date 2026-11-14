import { test } from 'node:test';
import assert from 'node:assert/strict';
import { textOf } from '@lingxia/view/testing';
import { renderExample } from '../testing/harness.mjs';

test('Reminder: a time editor named by its label; a picked time reaches the cell', async () => {
  const screen = await renderExample('TimeInput/TimeInput', 'Reminder');
  const input = screen.getByTestId('remind');
  assert.equal(input.tag, 'input');
  assert.equal(input.props.accessibilityLabel, 'Remind me at');
  assert.equal(input.props.keyboard, 'time');
  screen.type(input, '07:30');
  assert.equal(textOf(screen.getByTestId('remind-value')), 'Set for 07:30');
});

test('QuietHours: step and bounds reach the picker', async () => {
  const screen = await renderExample('TimeInput/TimeInput', 'QuietHours');
  const input = screen.getByTestId('quiet');
  assert.equal(input.props.inputStep, 900);
  assert.equal(input.props.inputMin, '18:00');
  assert.equal(input.props.inputMax, '23:45');
});

test('the editor leaves its width to the View, with or without seconds, and cuts a long placeholder with an ellipsis', async () => {
  const screen = await renderExample('TimeInput/TimeInput', 'Reminder');
  assert.equal(screen.getByTestId('remind').props.minWidthCh, undefined);
  assert.equal(screen.getByTestId('remind').props.textOverflow, 'ellipsis');
  screen.type(screen.getByTestId('remind'), '07:30:15');
  assert.equal(screen.getByTestId('remind').props.minWidthCh, undefined);
});
