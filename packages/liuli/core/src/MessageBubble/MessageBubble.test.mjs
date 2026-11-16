import { test } from 'node:test';
import assert from 'node:assert/strict';
import { textOf } from '@lingxia/view/testing';
import { renderExample } from '../testing/harness.mjs';

const bubbleOf = row => row.children.at(-1).children.find(node => node.props.background !== undefined);

test('Exchange: theirs at the start on a quiet tint with a name; mine at the end in the accent\'s soft step', async () => {
  const screen = await renderExample('MessageBubble/MessageBubble', 'Exchange');
  const [theirs, mine] = [screen.getByTestId('theirs'), screen.getByTestId('mine')];
  assert.deepEqual([theirs.props.justify, bubbleOf(theirs).props.background, textOf(theirs.children[1].children[0])], ['start', 'text/6', 'Kai Chen']);
  assert.deepEqual([mine.props.justify, bubbleOf(mine).props.background, bubbleOf(mine).props.color], ['end', 'accent-soft', 'on-accent-soft']);
});

test('Exchange: a bubble takes at most four fifths of the row and 520 px, and a long address breaks', async () => {
  const row = (await renderExample('MessageBubble/MessageBubble', 'Exchange')).getByTestId('long');
  const bubble = bubbleOf(row);
  assert.deepEqual([row.children[0].props.maxWidthPercent, bubble.props.maxWidth], [80, 520]);
  assert.equal(bubble.children[0].props.overflowWrap, 'anywhere');
});

test('States: a failed message says so in words with an error mark, and offers another try', async () => {
  const screen = await renderExample('MessageBubble/MessageBubble', 'States');
  const failed = screen.getByTestId('failed');
  const meta = failed.children[0].children.at(-1);
  assert.deepEqual([meta.children[0].tag, textOf(meta.children[1]), bubbleOf(failed).props.borderColor], ['vector', 'Not sent', 'danger']);
  screen.press(screen.getByTestId('retry'));
  screen.getByText('Not sent, tried 2 times');
});

test('Reasoning: one line that opens and closes its text; while streaming it reports busy', async () => {
  const screen = await renderExample('MessageBubble/MessageBubble', 'Reasoning');
  const toggle = screen.getByTestId('reasoning');
  assert.equal(toggle.props.accessibilityLabel, 'Thought for 8 seconds');
  assert.equal(toggle.props.stateExpanded, 1);
  screen.press(toggle);
  assert.equal(screen.getByTestId('reasoning').props.stateExpanded, 0);
  const thinking = screen.getByTestId('thinking');
  assert.equal(thinking.props.stateExpanded, 0);
  assert.ok(thinking.props.stateBusy ?? thinking.props.busy ?? thinking.props.accessibilityBusy, 'busy while streaming');
});
