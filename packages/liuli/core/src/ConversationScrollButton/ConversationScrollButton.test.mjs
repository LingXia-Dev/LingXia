import { test } from 'node:test';
import assert from 'node:assert/strict';
import { textOf } from '@lingxia/view/testing';
import { renderExample } from '../testing/harness.mjs';

const edge = (screen, at) => screen.fire(screen.getByTestId('messages'), 'onScrollEdge', JSON.stringify({ edge: 'end', at }));
const anchor = screen => JSON.parse(screen.getByTestId('messages').props.listAnchor);
const seq = screen => anchor(screen).seq;

test('Follow: at the newest message there is no button, and each new message moves the anchor', async () => {
  const screen = await renderExample('ConversationScrollButton/ConversationScrollButton', 'Follow');
  assert.throws(() => screen.getByTestId('latest'));
  const before = seq(screen);
  screen.press(screen.getByTestId('arrive'));
  assert.notEqual(seq(screen), before, 'the List follows');
});

test('Follow: reading further up shows the button and counts what arrives without moving; pressing it goes back', async () => {
  const screen = await renderExample('ConversationScrollButton/ConversationScrollButton', 'Follow');
  edge(screen, false);
  assert.equal(screen.getByTestId('latest').props.accessibilityLabel, 'Jump to the latest message');
  const [held, heldAt] = [seq(screen), anchor(screen).index];
  screen.press(screen.getByTestId('arrive'));
  screen.press(screen.getByTestId('arrive'));
  // A List may scroll on a new index as on a new seq: neither moves.
  assert.deepEqual([seq(screen), anchor(screen).index], [held, heldAt], 'the reader stays where they are');
  assert.equal(screen.getByTestId('latest').props.accessibilityLabel, 'Jump to the latest message, 2 new');
  assert.equal(textOf(screen.getByTestId('latest')), '2');
  screen.press(screen.getByTestId('latest'));
  assert.notEqual(seq(screen), held, 'the List goes to the newest message');
  assert.equal(anchor(screen).index, heldAt + 2);
  edge(screen, true);
  assert.throws(() => screen.getByTestId('latest'));
});
