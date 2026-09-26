import { waitForCurrentPage } from '../helpers/page.js';
import { expect, spec } from '@lingxia/test';
import { bindFixture, specNamespace } from '../helpers/poll.js';
import { SHOWCASE_APP_ID, rawApp } from '../helpers/app.js';

// String scripts and raw page reads go to the raw driver; see `rawApp`.
const raw = rawApp();

spec("open every component demo through rendered UI and the Logic bridge", { id: "COMPONENTS-001", covers: ['lx.navigateTo'], app: SHOWCASE_APP_ID }, async (t) => {
  const { app } = bindFixture(t, "COMPONENTS-001");

  const destinations = [
    ['components-video', 'video'],
    ['components-swiper', 'swiper'],
    ['components-navigator', 'navigator'],
    ['components-picker', 'picker'],
  ] as const;

  await app.nav.relaunch({ page: 'components' });
  await raw.page.waitFor({ page: 'components', css: '[data-testid="components-page"]' });

  for (const [testId, destination] of destinations) {
    await app.view.css(`[data-testid="${testId}"]`, { page: 'components' }).click();
    await waitForCurrentPage(raw, destination, 30_000);
    expect((await app.nav.current()).name).toBe(destination);

    await app.nav.back();
    await waitForCurrentPage(raw, 'components', 30_000);
    await raw.page.waitFor({ page: 'components', css: `[data-testid="${testId}"]` });
  }
});
