import { test } from 'node:test';
import assert from 'node:assert/strict';
import { renderExample } from '../testing/harness.mjs';

test('Person: the trigger is a button named by its words that shows a hint card on interest and on a press', async () => {
  const screen = await renderExample('HoverCard/HoverCard', 'Person');
  const trigger = screen.getByTestId('ana');
  assert.deepEqual([trigger.tag, trigger.props.accessibilityLabel], ['button', 'Ana Lima']);
  assert.ok(trigger.props.interestFor, 'shown while a pointer or focus rests on it');
  assert.ok(trigger.props.onClick, 'and by a press');
});

test('Term: the card opens at the side of its trigger', async () => {
  const screen = await renderExample('HoverCard/HoverCard', 'Term');
  screen.press(screen.getByTestId('seat'));
  const card = screen.getByTestId('seat-card');
  assert.ok(JSON.stringify(card.props).includes('inline-end'));
});
