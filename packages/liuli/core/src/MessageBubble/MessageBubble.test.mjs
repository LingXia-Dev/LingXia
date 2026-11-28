import { test } from 'node:test';
import assert from 'node:assert/strict';
import { textOf } from '@lingxia/view/testing';
import { renderExample } from '../testing/harness.mjs';

const lineOf = row => row.children[0];
const bubbleOf = row => lineOf(row).children.at(-1).children.find(node => node.props.background !== undefined);

test('Exchange: theirs at the start on a quiet tint with a name; mine at the end in the accent\'s soft step', async () => {
  const screen = await renderExample('MessageBubble/MessageBubble', 'Exchange');
  const [theirs, mine] = [screen.getByTestId('theirs'), screen.getByTestId('mine')];
  assert.deepEqual([lineOf(theirs).props.justify, bubbleOf(theirs).props.background, textOf(lineOf(theirs).children[1].children[0])], ['start', 'text/6', 'Kai Chen']);
  assert.deepEqual([lineOf(mine).props.justify, bubbleOf(mine).props.background, bubbleOf(mine).props.color], ['end', 'accent-soft', 'on-accent-soft']);
});

test('Exchange: a bubble takes at most four fifths of the row and 520 px, and a long address breaks', async () => {
  const row = (await renderExample('MessageBubble/MessageBubble', 'Exchange')).getByTestId('long');
  const bubble = bubbleOf(row);
  assert.deepEqual([lineOf(row).children.at(-1).props.maxWidthPercent, bubble.props.maxWidth], [80, 520]);
  assert.equal(bubble.children[0].props.overflowWrap, 'anywhere');
});

test('States: a failed message says so in words with an error mark, and offers another try', async () => {
  const screen = await renderExample('MessageBubble/MessageBubble', 'States');
  const failed = screen.getByTestId('failed');
  const meta = failed.children.at(-1);
  assert.deepEqual([meta.children[0].tag, textOf(meta.children[1]), bubbleOf(failed).props.borderColor], ['vector', 'Not sent', undefined], 'no red edge on the bubble: the mark and the words say it');
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

test('ToolCalls: each step is named by its label and where it stands; one with details opens, one waiting asks to be allowed', async () => {
  const screen = await renderExample('MessageBubble/MessageBubble', 'ToolCalls');
  const searched = screen.getByTestId('searched');
  assert.deepEqual([searched.props.accessibilityLabel, searched.props.stateExpanded], ["Searched the notes for 'packing list', done, 3 results", 0]);
  screen.press(searched);
  assert.equal(screen.getByTestId('searched').props.stateExpanded, 1);
  const tests = screen.getByTestId('tests');
  assert.equal(tests.props.accessibilityLabel, 'npm test, failed, 4.2 s');
  screen.getByText('Failed');
  const forecast = screen.getByTestId('forecast');
  assert.equal(forecast.props.stateExpanded, undefined, 'no details, so nothing to open');
  assert.ok(forecast.props.stateBusy ?? forecast.props.busy ?? forecast.props.accessibilityBusy, 'busy while running');
  assert.equal(screen.getByTestId('delete').props.accessibilityLabel, 'Delete 3 old drafts in Notes, waiting for your answer');
  screen.getByTestId('delete-details');
  assert.equal(textOf(screen.getByTestId('delete-allow')), 'Delete 3 drafts');
  screen.press(screen.getByTestId('delete-deny'));
  assert.equal(screen.getByTestId('delete').props.accessibilityLabel, 'Delete 3 old drafts in Notes, denied');
  screen.getByText('Denied');
  assert.throws(() => screen.getByTestId('delete-allow'), 'answered, the buttons go');
});

test('ToolCalls: in Chinese, the state is read in the page\'s language', async () => {
  const screen = await renderExample('MessageBubble/MessageBubble', 'ToolCalls', { env: { language: 'zh-CN' } });
  assert.equal(screen.getByTestId('tests').props.accessibilityLabel, 'npm test，失败，4.2 s');
  assert.equal(textOf(screen.getByTestId('delete-deny')), '拒绝');
});
