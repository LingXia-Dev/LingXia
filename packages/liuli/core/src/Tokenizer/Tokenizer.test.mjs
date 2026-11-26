import { test } from 'node:test';
import assert from 'node:assert/strict';
import { textOf } from '@lingxia/view/testing';
import { renderExample } from '../testing/harness.mjs';

const tokens = (screen, id) => screen.queryAll(node => node.testId?.startsWith(`${id}-token-`) && !node.testId.endsWith('-remove')).map(textOf);

test('Tags: a comma or Enter turns the text into a token; a pasted list splits; a repeat is not added', async () => {
  const screen = await renderExample('Tokenizer/Tokenizer', 'Tags');
  const editor = () => screen.getByTestId('tags');
  assert.deepEqual([screen.getByTestId('tags-box').props.role, editor().props.accessibilityLabel], ['group', 'Tags']);
  screen.type(editor(), 'harbour, isl');
  assert.deepEqual(tokens(screen, 'tags'), ['ferries', 'summer', 'harbour']);
  assert.equal(editor().props.inputValue, 'isl', 'the unfinished entry stays');
  screen.type(editor(), 'island');
  screen.key(editor(), 'Enter');
  assert.deepEqual(tokens(screen, 'tags'), ['ferries', 'summer', 'harbour', 'island']);
  assert.equal(editor().props.inputValue, '');
  screen.type(editor(), 'boats; summer\npier,');
  assert.deepEqual(tokens(screen, 'tags'), ['ferries', 'summer', 'harbour', 'island', 'boats', 'pier']);
  const remove = screen.getByTestId('tags-token-0-remove');
  assert.equal(remove.props.accessibilityLabel, 'Remove ferries');
  screen.press(remove);
  assert.equal(tokens(screen, 'tags')[0], 'summer');
});

test('Recipients: a refused entry stays in the editor with the reason as the field\'s error; leaving keeps a good one', async () => {
  const screen = await renderExample('Tokenizer/Tokenizer', 'Recipients');
  const editor = () => screen.getByTestId('invite');
  screen.type(editor(), 'sam@pier.co, ana');
  screen.key(editor(), 'Enter');
  assert.deepEqual(tokens(screen, 'invite'), ['mira@harbour.co', 'sam@pier.co']);
  assert.equal(editor().props.inputValue, 'ana');
  assert.ok(screen.getByText('"ana" is not an email address.'));
  screen.type(editor(), 'ana@lima.co');
  assert.equal(screen.queryAll(node => textOf(node) === '"ana" is not an email address.').length, 0, 'typing clears the reason');
  screen.blur(editor());
  assert.deepEqual(tokens(screen, 'invite'), ['mira@harbour.co', 'sam@pier.co', 'ana@lima.co']);
});
