import { test } from 'node:test';
import assert from 'node:assert/strict';
import { renderExample } from '../testing/harness.mjs';

// The page pads its content by `env.insets`; a bar that added them again would double the gap.
for (const sizeClass of ['compact', 'regular']) {
  test(`TopBar ${sizeClass}: adds no inset padding of its own`, async () => {
    const screen = await renderExample('TopBar/TopBar', 'StandardTopBar', {
      env: {
        sizeClass,
        direction: 'ltr',
        insets: { top: 0, right: 80, bottom: 0, left: 40 },
        viewport: sizeClass === 'compact' ? { width: 393, height: 852 } : { width: 1024, height: 768 },
      },
    });
    const bar = screen.getByTestId('orders-topbar');
    assert.equal(bar.props.paddingInlineStart, undefined);
    assert.equal(bar.props.paddingInlineEnd, undefined);
  });
}
