import { test } from 'node:test';
import assert from 'node:assert/strict';
import { renderExample } from '../testing/harness.mjs';

test('Between: a hairline across, one pixel tall, not read', async () => {
  const line = (await renderExample('Divider/Divider', 'Between')).getByTestId('line');
  assert.deepEqual([line.props.height, line.props.background, line.props.accessibilityHidden, line.props.alignSelf], [1, 'line', 1, 'stretch']);
});

test('Labelled: the word is read where it stands, between two lines that are not', async () => {
  const row = (await renderExample('Divider/Divider', 'Labelled')).getByTestId('or');
  assert.deepEqual(row.children.map(node => (node.tag === 'text' ? node.text : node.props.accessibilityHidden)), [1, 'or', 1]);
  assert.deepEqual(row.children.map(node => node.props.grow ?? 0), [1, 0, 1]);
});

test('Vertical: one pixel wide and as tall as its row', async () => {
  const line = (await renderExample('Divider/Divider', 'Vertical')).getByTestId('upright');
  assert.deepEqual([line.props.width, line.props.alignSelf, line.props.accessibilityHidden], [1, 'stretch', 1]);
});
