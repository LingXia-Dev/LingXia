import { test } from 'node:test';
import assert from 'node:assert/strict';
import { textOf } from '@lingxia/view/testing';
import { renderExample } from '../testing/harness.mjs';

test('Invoice: a named list; each pair is read as one phrase, its label and value drawn only', async () => {
  const screen = await renderExample('MetadataList/MetadataList', 'Invoice');
  const list = screen.getByTestId('invoice');
  assert.deepEqual([list.props.role, list.props.accessibilityLabel, list.children.length], ['list', 'Invoice details', 5]);
  const due = screen.getByTestId('invoice-due');
  assert.deepEqual([due.props.role, due.props.accessibilityLabel], ['listitem', 'Due: 9 November 2026']);
  assert.ok(due.children.every(node => node.props.accessibilityHidden === 1));
  const [label, value] = due.children;
  assert.deepEqual([label.props.color, label.props.fontSize, label.props.flexBasis, label.props.shrink], ['text-muted', 13, '33.333333%', 0], 'the labels share a third of the row');
  assert.equal(label.props.widthPercent, undefined, 'a basis, not a width: at large text a word wider than a third widens the label instead of running into the value');
  assert.deepEqual([value.props.color, value.props.grow], ['text', 1]);
});

test('Invoice: a value drawn as a badge is still read as its text', async () => {
  const screen = await renderExample('MetadataList/MetadataList', 'Invoice');
  const status = screen.getByTestId('invoice-status');
  assert.equal(status.props.accessibilityLabel, 'Status: Overdue');
  assert.equal(textOf(status.children[1]), 'Overdue');
  assert.equal(status.children[1].tag === 'text', false, 'drawn by its content');
});

test('Stacked: each label over its value, two to a line', async () => {
  const screen = await renderExample('MetadataList/MetadataList', 'Stacked');
  const list = screen.getByTestId('file');
  assert.match(String(list.props.gridTemplateColumns), /repeat\(2|1fr 1fr|^2$/);
  assert.deepEqual(screen.getByTestId('file-kind').children.map(node => node.text), ['Kind', 'PDF document']);
});
