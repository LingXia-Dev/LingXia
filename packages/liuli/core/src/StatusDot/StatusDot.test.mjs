import { test } from 'node:test';
import assert from 'node:assert/strict';
import { textOf } from '@lingxia/view/testing';
import { renderExample } from '../testing/harness.mjs';

test('Services: the word is read and the dot is not; the dot takes the mark colour, neutral a ring', async () => {
  const screen = await renderExample('StatusDot/StatusDot', 'Services');
  const [dot, word] = screen.getByTestId('sync').children;
  assert.deepEqual([dot.props.background, dot.props.accessibilityHidden, dot.props.width, textOf(word)], ['success-mark', 1, 8, 'Running']);
  const ring = screen.getByTestId('backups').children[0];
  assert.deepEqual([ring.props.background, ring.props.borderColor], [undefined, 'line-strong']);
});

test('DotOnly: without its words the dot is an image named by them', async () => {
  const dot = (await renderExample('StatusDot/StatusDot', 'DotOnly')).getByTestId('bare');
  assert.deepEqual([dot.props.role, dot.props.accessibilityLabel], ['img', 'Online']);
});
