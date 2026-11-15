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
