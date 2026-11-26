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

test('info and hues: the info mark, and a hue\'s mark in place of a tone', async () => {
  const info = (await renderExample('StatusDot/StatusDot', 'Tones')).getByTestId('info');
  assert.equal(info.children[0].props.background, 'info-mark');
  const hue = (await renderExample('StatusDot/StatusDot', 'Hues')).getByTestId('hue-1');
  assert.equal(hue.children[0].props.background, 'hue-1');
  assert.ok(hue.children[0].props.accessibilityHidden);
});

test('Live: a pulsing dot holds a halo in its own colour that rings out only when motion is allowed, and is not read', async () => {
  const screen = await renderExample('StatusDot/StatusDot', 'Live');
  const [dot, word] = screen.getByTestId('recording').children;
  assert.equal(textOf(word), 'Recording');
  const halo = dot.children[0];
  assert.equal(halo.props.background, 'danger-mark');
  assert.equal(halo.props.accessibilityHidden, 1);
  assert.ok(JSON.stringify(halo.props).includes('ping'), 'it rings out');
});
