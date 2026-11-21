import { expect, test } from '@lingxia/test';
import type { PageContract } from '@lingxia/types/page';
import { SHOWCASE_APP_ID } from '@lingxia/test/showcase/helpers/app.js';
import { harmonyDevice } from '@lingxia/test/showcase/helpers/harmony-device.js';
import { bindFixture } from '@lingxia/test/showcase/helpers/poll.js';

// A pull through the system's input path (`uitest uiInput swipe`), not injected touches: the
// GPU page must give up the drag at its top so the host's refresh takes it.
test('pull down from the screen to refresh, then refresh again', {
  id: 'HARMONY-PULL-GESTURE-001', app: SHOWCASE_APP_ID,
  covers: ['page.refresh', 'PageSetup.onRefresh'],
  requires: { args: ['harmonyDevice'] },
  reason: 'requires the Harmony device fixture',
  start: { page: 'pullToRefresh' },
}, async (t) => {
  const { app } = bindFixture(t, 'HARMONY-PULL-GESTURE-001');
  const page = await app.page<PageContract<{ refreshCount: number; isRefreshing: boolean }>>({ name: 'pullToRefresh' });
  const view = page.view;
  const device = harmonyDevice(t);
  const screen = (await device.nodes()).reduce((a, b) => (b.bounds[2] * b.bounds[3] > a.bounds[2] * a.bounds[3] ? b : a));
  const [, , width, height] = screen.bounds;
  await expect(view.testId('pull-refresh-page')).toBeVisible();
  for (let gesture = 0; gesture < 2; gesture++) {
    const before = (await page.data()).refreshCount;
    await device.swipe({ x1: Math.round(width / 2), x2: Math.round(width / 2),
      y1: Math.round(height * .22), y2: Math.round(height * .75), velocity: 1200 });
    await expect.poll(async () => (await page.data()).refreshCount).toBe(before + 1);
    await expect(view.testId('pull-refresh-count')).toHaveText(String(before + 1));
    await view.testId('pull-refresh-stop').click();
    await expect.poll(async () => (await page.data()).isRefreshing).toBe(false);
    await expect(view.testId('pull-refresh-status')).toContainText('Idle');
  }
});
