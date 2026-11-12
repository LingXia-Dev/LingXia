import { test } from 'node:test';
import assert from 'node:assert/strict';
import { textOf } from '@lingxia/view/testing';
import { described, renderExample } from '../testing/harness.mjs';

test('Tones: soft tints, text in the text role except on accent', async () => {
  const screen = await renderExample('Badge/Badge', 'Tones');
  const look = id => [screen.getByTestId(id).props.background, screen.getByTestId(id).props.color];
  assert.deepEqual(look('neutral'), ['surface-sunken', 'text']);
  assert.deepEqual(look('accent'), ['accent-soft', 'on-accent-soft']);
  assert.deepEqual(look('danger'), ['danger-soft', 'text']);
  assert.equal(textOf(screen.getByTestId('danger')), 'Overdue');
  assert.equal(screen.getByTestId('danger').props.role, undefined, 'plain text, not a control');
});

test('Solid: fill with its on- role', async () => {
  const screen = await renderExample('Badge/Badge', 'Solid');
  assert.equal(screen.getByTestId('solid').props.background, 'accent');
  assert.equal(screen.getByTestId('solid').props.color, 'on-accent');
});

test('Sizes: heights 20 and 24; the icon is hidden', async () => {
  const screen = await renderExample('Badge/Badge', 'Sizes');
  assert.equal(screen.getByTestId('small').props.height, 20);
  const icon = screen.queryAll(node => node.props.accessibilityHidden && node.tag !== 'vector');
  assert.equal(icon.length, 1);
});

test('InARow: the row is read as its label and description; the badge is drawn only', async () => {
  const screen = await renderExample('Badge/Badge', 'InARow');
  const row = screen.getByTestId('invoice');
  assert.equal(row.props.accessibilityLabel, 'Invoice 2041');
  assert.deepEqual(described(screen, row), ['Overdue, due 3 May']);
});
