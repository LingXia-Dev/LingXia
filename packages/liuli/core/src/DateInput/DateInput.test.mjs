import { test } from 'node:test';
import assert from 'node:assert/strict';
import { textOf } from '@lingxia/view/testing';
import { renderExample } from '../testing/harness.mjs';

test('DueDate: a date editor named by its label; a picked day reaches the cell', async () => {
  const screen = await renderExample('DateInput/DateInput', 'DueDate');
  const input = screen.getByTestId('due');
  assert.equal(input.tag, 'input');
  assert.equal(input.props.accessibilityLabel, 'Due date');
  assert.equal(input.props.keyboard, 'date');
  screen.type(input, '2026-12-01');
  assert.equal(textOf(screen.getByTestId('due-value')), 'Due 2026-12-01');
});

test('Trip: the return takes the departure as its earliest day and is marked before it', async () => {
  const screen = await renderExample('DateInput/DateInput', 'Trip');
  assert.equal(screen.getByTestId('back').props.inputMin, '2026-12-18');
  screen.type(screen.getByTestId('back'), '2026-12-10');
  assert.equal(screen.getByTestId('back').props.stateInvalid, 1);
  screen.getByText('Return after you leave.');
});

test('the editor leaves its width to the View and cuts a long placeholder with an ellipsis', async () => {
  const input = (await renderExample('DateInput/DateInput', 'DueDate')).getByTestId('due');
  assert.equal(input.props.minWidthCh, undefined);
  assert.equal(input.props.fontVariantNumeric, 'tabular-nums');
  assert.equal(input.props.textOverflow, 'ellipsis');
});

test('Meeting: a day and a time share a row, each growing from zero beside the other', async () => {
  const screen = await renderExample('DateInput/DateInput', 'Meeting');
  assert.equal(screen.getByTestId('meeting-day').props.keyboard, 'date');
  assert.equal(screen.getByTestId('meeting-time').props.keyboard, 'time');
  assert.equal(screen.getByTestId('meeting-day').props.minWidthCh, undefined);
});

test('Clearable: a clear button named "Clear Due date" empties the field, and is gone once it is empty', async () => {
  const screen = await renderExample('DateInput/DateInput', 'Clearable');
  const clear = screen.getByTestId('clearable-clear');
  assert.equal(clear.props.accessibilityLabel, 'Clear Due date');
  screen.press(clear);
  assert.equal(screen.queryAll(node => node.testId === 'clearable-clear').length, 0, 'gone once empty');
});
