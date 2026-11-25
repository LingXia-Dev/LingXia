import { test } from 'node:test';
import assert from 'node:assert/strict';
import { textOf } from '@lingxia/view/testing';
import { renderExample } from '../testing/harness.mjs';

test('Recipients: a named group of tokens; a remove button says what it removes and takes it away', async () => {
  const screen = await renderExample('Token/Token', 'Recipients');
  const group = screen.getByTestId('shared');
  assert.deepEqual([group.props.role, group.props.accessibilityLabel, group.children.length], ['group', 'Shared with', 3]);
  const remove = screen.getByTestId('person-kai-remove');
  assert.deepEqual([remove.tag, remove.props.accessibilityLabel, remove.props.width], ['button', 'Remove Kai Chen', 32]);
  screen.press(remove);
  assert.deepEqual(screen.getByTestId('shared').children.map(textOf), ['Ana Lima', 'Mira Patel']);
  screen.getByText('2 people');
});

test('Filters: a pressable token is one button that reports pressed and changes its fill', async () => {
  const screen = await renderExample('Token/Token', 'Filters');
  const token = id => screen.getByTestId(id);
  assert.deepEqual([token('filter-unread').tag, token('filter-unread').props.background, token('filter-shared').props.background], ['button', 'accent-soft', 'text/8']);
  screen.press(token('filter-shared'));
  assert.equal(token('filter-shared').props.background, 'accent-soft');
});

test('Labels: a hue\'s soft fill with a hidden dot in the hue; the label is still what is read', async () => {
  const screen = await renderExample('Token/Token', 'Labels');
  const token = screen.getByTestId('label-bug');
  assert.equal(token.props.background, 'hue-5-soft');
  assert.equal(token.children[0].props.background, 'hue-5');
  assert.ok(token.children[0].props.accessibilityHidden);
  assert.equal(screen.getByTestId('label-bug-remove').props.accessibilityLabel, 'Remove bug');
});

test('StateDisabled: faded by an opaque colour, remove button too', async () => {
  const screen = await renderExample('Token/Token', 'StateDisabled');
  const root = screen.queryAll(node => node.props.background === 'text/5')[0];
  assert.ok(root, 'the pill fades its fill');
  assert.equal(root.props.opacity, undefined);
  assert.equal(root.props.color, 'text-faint');
  const remove = screen.queryAll(node => node.tag === 'button' && node.props.accessibilityLabel === 'Remove Locked')[0];
  assert.equal(remove.props.color, 'text-faint');
});

test('Recipients at large text: the icon and the remove cross grow with the label, up to half again, the target with them', async () => {
  for (const [textScale, growth] of [[1, 1], [2, 1.5]]) {
    const screen = await renderExample('Token/Token', 'Recipients', { env: { textScale } });
    const remove = screen.getByTestId('person-ana-remove');
    const cross = Math.round(14 * growth);
    assert.deepEqual([remove.props.width ?? 32, remove.children[0].props.width ?? 14], [32 + cross - 14, cross], `at ${textScale}`);
  }
});
