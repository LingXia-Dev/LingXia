import './shared-setup.js';
import { expect, spec } from '@lingxia/test';
import type { PageContract } from '@lingxia/types/page';
import { SHOWCASE_APP_ID } from '@lingxia/test/showcase/helpers/app.js';
import { androidDevice } from '@lingxia/test/showcase/helpers/android-device.js';
import { bindFixture } from '@lingxia/test/showcase/helpers/poll.js';

// A pull through the OS input dispatcher (`adb shell input swipe`), not the injected touches
// the other pull specs use: the path a finger takes, system touch slop and batching included.
spec('pull down from the screen to refresh, then refresh again', {
  id: 'ANDROID-PULL-GESTURE-001', app: SHOWCASE_APP_ID,
  covers: ['lx.startPullDownRefresh', 'lx.stopPullDownRefresh'],
  requires: { args: ['androidDevice'] },
  reason: 'requires the Android OS input fixture',
  start: { page: 'pullToRefresh' },
}, async (t) => {
  const { app } = bindFixture(t, 'ANDROID-PULL-GESTURE-001');
  const page = await app.page<PageContract<{ refreshCount: number; isRefreshing: boolean }>>({ name: 'pullToRefresh' });
  const view = page.view;
  const device = androidDevice(t);
  const { width, height } = await device.size();
  await expect(view.testId('pull-refresh-page')).toBeVisible();
  for (let gesture = 0; gesture < 2; gesture++) {
    const before = (await page.data()).refreshCount;
    await device.swipe({ x1: Math.round(width / 2), x2: Math.round(width / 2),
      y1: Math.round(height * .22), y2: Math.round(height * .75), duration: 650 });
    await expect.poll(async () => (await page.data()).refreshCount).toBe(before + 1);
    await expect(view.testId('pull-refresh-count')).toHaveText(String(before + 1));
    await view.testId('pull-refresh-stop').click();
    await expect.poll(async () => (await page.data()).isRefreshing).toBe(false);
    await expect(view.testId('pull-refresh-status')).toContainText('Idle');
  }
});
