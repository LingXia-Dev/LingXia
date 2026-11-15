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
