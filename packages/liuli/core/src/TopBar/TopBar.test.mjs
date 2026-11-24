import { test } from 'node:test';
import assert from 'node:assert/strict';
import { textOf } from '@lingxia/view/testing';
import { renderExample } from '../testing/harness.mjs';

test('Standard: the way back, then the title as a heading, then the actions at the end', async () => {
  const screen = await renderExample('TopBar/TopBar', 'Standard');
  const bar = screen.getByTestId('orders-topbar');
  const [start, end] = bar.children;
  assert.equal(start.children[0], screen.getByTestId('orders-back'));
  const heading = start.children[1];
  assert.deepEqual([heading.props.role, textOf(heading)], ['heading', 'Orders']);
  assert.deepEqual(end.children.map(node => node.props.accessibilityLabel), ['Filter', 'New order']);
});

test('The start and the end share the room left over, so the centre sits at the middle', async () => {
  const screen = await renderExample('TopBar/TopBar', 'WithSearch');
  const [start, center, end] = screen.getByTestId('mail-topbar').children;
  assert.deepEqual([start.props.grow, start.props.flexBasis, end.props.grow, end.props.flexBasis], [1, 0, 1, 0]);
  assert.equal(center.props.shrink, 0);
});

// The page pads its content by `env.insets`; a bar that added them again would double the gap.
test('Adds no inset of its own at either size', async () => {
  for (const sizeClass of ['compact', 'regular']) {
    const screen = await renderExample('TopBar/TopBar', 'Standard', {
      env: { sizeClass, direction: 'ltr', insets: { top: 0, right: 80, bottom: 0, left: 40 }, viewport: sizeClass === 'compact' ? { width: 393, height: 852 } : { width: 1024, height: 768 } },
    });
    const bar = screen.getByTestId('orders-topbar');
    assert.equal(bar.props.paddingStart ?? bar.props.paddingInlineStart ?? bar.props.paddingLeft, 12, sizeClass);
  }
});
