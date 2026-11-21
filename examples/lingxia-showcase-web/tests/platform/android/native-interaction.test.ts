import { expect, test } from '@lingxia/test';
import type { PageContract } from '@lingxia/types/page';
import { APP_ID } from '../../support/app.js';
import { androidDevice } from '../../support/android-device.js';
import { bindFixture } from '../../support/poll.js';

const args = globalThis.__LINGXIA_AUTOMATION_HOST__?.args ?? {} as Record<string, string>;
test.configure({ skip: args.platform === 'android' ? undefined : 'Android only' });

test('pull down from the screen to refresh, then refresh again', {
  id: 'ANDROID-PULL-GESTURE-001', app: APP_ID,
  covers: ['page.refresh', 'PageSetup.onPullDownRefresh'],
  requires: { args: ['androidDevice'] },
  reason: 'requires Android OS input fixture',
  start: { page: 'pullToRefresh' },
}, async (t) => {
  const { app, defer } = bindFixture(t, 'ANDROID-PULL-GESTURE-001');
  defer(async () => { await app.nav.reset({ page: 'home' }); });
  const page = await app.page<PageContract<{ refreshCount: number; isRefreshing: boolean }>>({ name: 'pullToRefresh' });
  const device = androidDevice(t);
  const { width, height } = await device.size();
  await expect(page.view.testId('pull-refresh-page')).toBeVisible();
  for (let gesture = 0; gesture < 2; gesture++) {
    // A stopped indicator lingers and springs back; a pull before it is idle is ignored.
    await expect.poll(async () => (await page.view.refreshIndicator()).phase, { timeout: 3_000 }).toBe('idle');
    const before = (await page.data()).refreshCount;
    await device.swipe({ x1: Math.round(width / 2), x2: Math.round(width / 2),
      y1: Math.round(height * .22), y2: Math.round(height * .75), duration: 650 });
    try {
      await expect.poll(async () => (await page.data()).refreshCount).toBe(before + 1);
    } catch (error) {
      // An idle indicator that ignored the pull is the host's; say so in the report.
      await t.attach(`indicator-${gesture + 1}.json`, JSON.stringify(await page.view.refreshIndicator()));
      throw error;
    }
    await expect(page.view.testId('pull-refresh-count')).toHaveText(String(before + 1));
    // The refresh indicator translates the native WebView container; use its
    // screen bounds instead of a DOM click while that container is moving.
    await device.tapText('Finish refresh', 'android.widget.Button');
    await expect.poll(async () => (await page.data()).isRefreshing).toBe(false);
    await expect(page.view.testId('pull-refresh-status')).toContainText('Idle');
  }
});
