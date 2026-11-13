import { test } from 'node:test';
import assert from 'node:assert/strict';
import { renderExample } from '../testing/harness.mjs';

test('ListLoading: one busy status region named by what is loading; every shape hidden', async () => {
  const screen = await renderExample('Skeleton/Skeleton', 'ListLoading');
  const group = screen.getByTestId('loading-notes');
  assert.equal(group.props.role, 'status');
  assert.equal(group.props.accessibilityLabel, 'Loading notes');
  assert.equal(group.props.stateBusy, 1);
  const statuses = screen.queryAll(node => node.props.role === 'status');
  assert.equal(statuses.length, 1, 'said once, not per shape');
  const hidden = screen.queryAll(node => node.props.accessibilityHidden === 1);
  assert.equal(hidden.length, 9, 'three rows of a circle and two lines');
});

test('text lines: several lines, the last one shorter', async () => {
  const screen = await renderExample('Skeleton/Skeleton', 'CardLoading');
  const paragraph = screen.getByTestId('paragraph');
  assert.equal(paragraph.children.length, 3);
  const [first, , last] = paragraph.children;
  assert.equal(first.props.widthPercent, undefined);
  assert.equal(last.props.widthPercent, 60);
});

test('the glint rests out of view and moves only under motion-safe', async () => {
  const screen = await renderExample('Skeleton/Skeleton', 'Shapes');
  const glints = screen.queryAll(node => node.props.position === 'absolute');
  assert.equal(glints.length, 4, 'two lines, a circle and a block');
  for (const glint of glints) {
    assert.deepEqual(JSON.parse(glint.props.transform).translate, ['-100%', 0]);
    assert.equal(JSON.parse(glint.props.animation).name, 'skeleton-glint');
  }
});

test('reduced motion: no glint animation, the band stays out of view', async () => {
  const screen = await renderExample('Skeleton/Skeleton', 'Shapes', { env: { reducedMotion: true } });
  const glints = screen.queryAll(node => node.props.position === 'absolute');
  for (const glint of glints) {
    assert.deepEqual(JSON.parse(glint.props.transform).translate, ['-100%', 0]);
    assert.equal(glint.props.animation, undefined);
  }
});
