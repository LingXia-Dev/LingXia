import { expect, test } from '@lingxia/test';
import { APP_ID } from '../support/app.js';
import { waitForCurrentPage } from '../support/page.js';

test('open every demo from the home list through LxNavigator', {
  id: 'HOME-DEMOS-001', covers: ['lx.nav.go'], app: APP_ID,
}, async (t) => {
  const app = t.app;
  const destinations = [
    ['home-picker', 'picker'],
    ['home-swiper', 'swiper'],
    ['home-navigator', 'navigator'],
    ['home-video', 'video'],
    ['home-chat', 'chat'],
    ['home-channel', 'channel'],
    ['home-bridge-repro', 'bridge-repro'],
    ['home-pullToRefresh', 'pullToRefresh'],
  ] as const;

  await app.nav.reset({ page: 'home' });
  // Each demo is pushed above this instance and popped back to it.
  const home = (await app.page({ name: 'home' }, { timeout: 30_000 })).view;
  await home.testId('home-page').waitFor({ timeout: 30_000 });

  for (const [testId, destination] of destinations) {
    await home.testId(testId).click();
    await waitForCurrentPage(app, destination, 30_000);
    expect((await app.nav.current()).name).toBe(destination);

    await app.nav.back();
    await waitForCurrentPage(app, 'home', 30_000);
    await home.testId(testId).first().waitFor({ timeout: 30_000 });
  }
});
