import { test } from 'node:test';
import assert from 'node:assert/strict';
import { textOf } from '@lingxia/view/testing';
import { described, renderExample } from '../testing/harness.mjs';

test('Tones: soft tints, text in the text role except on accent', async () => {
  const screen = await renderExample('Badge/Badge', 'Tones');
  const look = id => [screen.getByTestId(id).props.background, screen.getByTestId(id).props.color];
  assert.deepEqual(look('neutral'), ['text/8', 'text']);
  assert.deepEqual(look('accent'), ['accent-soft', 'on-accent-soft']);
  assert.deepEqual(look('danger'), ['danger-soft', 'text']);
  assert.equal(textOf(screen.getByTestId('danger')), 'Overdue');
  assert.equal(screen.getByTestId('danger').props.role, undefined, 'plain text, not a control');
});

test('Solid: fill with its on- role', async () => {
  const screen = await renderExample('Badge/Badge', 'Solid');
  assert.equal(screen.getByTestId('solid').props.background, 'accent-solid');
  assert.equal(screen.getByTestId('solid').props.color, 'on-accent-solid');
});

test('Sizes: heights 20 and 24 at the least, grown by the label at large text; the icon is hidden', async () => {
  const screen = await renderExample('Badge/Badge', 'Sizes');
  const { props } = screen.getByTestId('small');
  assert.deepEqual([props.height, props.minHeight, props.paddingTop, props.paddingBottom], [undefined, 20, 2, 2]);
  // The icon is drawn from the set by its name, and not read.
  const icon = screen.queryAll(node => node.tag === 'vector' && node.props.accessibilityHidden);
  assert.equal(icon.length, 1);
});

test('InARow: the row is read as its label and description; the badge is drawn only', async () => {
  const screen = await renderExample('Badge/Badge', 'InARow');
  const row = screen.getByTestId('invoice');
  assert.equal(row.props.accessibilityLabel, 'Invoice 2041');
  assert.deepEqual(described(screen, row), ['Overdue, due 3 May']);
});

test('Tones: info is its own family, soft and solid', async () => {
  const soft = (await renderExample('Badge/Badge', 'Tones')).getByTestId('info');
  assert.deepEqual([soft.props.background, soft.props.color], ['info-soft', 'text']);
});

test('Hues: the hue\'s soft fill with text in the text role, and a hidden dot in the hue', async () => {
  const screen = await renderExample('Badge/Badge', 'Hues');
  const badge = screen.getByTestId('hue-1');
  assert.deepEqual([badge.props.background, badge.props.color], ['hue-1-soft', 'text']);
  const [dot] = badge.children;
  assert.equal(dot.props.background, 'hue-1');
  assert.ok(dot.props.accessibilityHidden);
  assert.equal(textOf(badge), 'Design');
});
