import { test } from 'node:test';
import assert from 'node:assert/strict';
import { renderExample } from '../testing/harness.mjs';

test('Grid: a thumbnail that opens is a button named by what it shows; its picture is not read twice', async () => {
  const tile = (await renderExample('Thumbnail/Thumbnail', 'Grid')).getByTestId('grid-0');
  assert.deepEqual([tile.tag, tile.props.accessibilityLabel, tile.children[0].tag, tile.props.alignSelf], ['button', 'The bay at sunrise', 'image', 'stretch']);
});

test('InRows: without a press it is a 48 px image tile', async () => {
  const tile = (await renderExample('Thumbnail/Thumbnail', 'InRows')).getByTestId('row');
  assert.deepEqual([tile.tag === 'button', tile.props.width, tile.children[0].tag], [false, 48, 'image']);
});

test('Strip: the one in view is ringed', async () => {
  const screen = await renderExample('Thumbnail/Thumbnail', 'Strip');
  assert.ok(String(screen.getByTestId('strip-1').props.ring).includes('accent'));
  assert.equal(screen.getByTestId('strip-0').props.ring ?? 'none', 'none');
});

test('Attachments: each has a remove button named for its picture, beside it rather than inside; one still loading is busy', async () => {
  const screen = await renderExample('Thumbnail/Thumbnail', 'Attachments');
  const remove = screen.getByTestId('attached-0-remove');
  assert.deepEqual([remove.tag, remove.props.accessibilityLabel], ['button', 'Remove The bay at sunrise']);
  const frame = screen.getByTestId('attached-0');
  assert.equal(frame.children[0].tag, 'column', 'the picture and the button are siblings');
  assert.equal(frame.children[1], remove);
  assert.equal(frame.children[0].children[0].tag, 'image');
  const loading = screen.getByTestId('attached-2');
  assert.equal(loading.children[0].props.stateBusy, 1, 'the uploading one is busy');
  screen.press(remove);
  assert.equal(screen.queryAll(node => node.props?.testId === 'attached-0').length, 0, 'pressed, it is taken away');
});
