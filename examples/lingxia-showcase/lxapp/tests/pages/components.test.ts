import { waitForCurrentPage } from '../helpers/page.js';
import { expect, spec } from '@lingxia/test';
import { bindFixture, specNamespace } from '../helpers/poll.js';
import { SHOWCASE_APP_ID } from '../helpers/app.js';


spec("open every component demo through rendered UI and the Logic bridge", { id: "COMPONENTS-001", covers: ['lx.navigateTo'], app: SHOWCASE_APP_ID }, async (t) => {
  const { app } = bindFixture(t, "COMPONENTS-001");

  const destinations = [
    ['components-video', 'video'],
    ['components-swiper', 'swiper'],
    ['components-navigator', 'navigator'],
    ['components-picker', 'picker'],
  ] as const;

  await app.nav.relaunch({ page: 'components' });
  // Each demo is pushed above this instance and popped back to it.
  const components = (await app.page({ name: 'components' }, { timeout: 30_000 })).view;
  await components.testId('components-page').waitFor({ timeout: 30_000 });

  for (const [testId, destination] of destinations) {
    await components.testId(testId).click();
    await waitForCurrentPage(app, destination, 30_000);
    expect((await app.nav.current()).name).toBe(destination);

    await app.nav.back();
    await waitForCurrentPage(app, 'components', 30_000);
    await components.testId(testId).first().waitFor({ timeout: 30_000 });
  }
});
