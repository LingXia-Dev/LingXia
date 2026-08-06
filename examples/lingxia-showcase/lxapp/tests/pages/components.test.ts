import { expect } from '@rongjs/test';
import { waitForCurrentPage } from '../helpers/page.js';
import { contract } from '../support/contract.js';

contract({
  id: 'COMPONENTS-001',
  title: 'open every component demo through rendered UI and the Logic bridge',
  covers: ['lx.navigateTo'],
  layer: 'logic',
  levels: ['semantic', 'boundary', 'lifecycle'],
  scope: 'portable',
  expectedOutcome: 'supported',
}, async ({ app }) => {
  const destinations = [
    ['components-video', 'video'],
    ['components-swiper', 'swiper'],
    ['components-navigator', 'navigator'],
    ['components-picker', 'picker'],
  ] as const;

  await app.nav.relaunch({ page: 'components' });
  await app.page.waitFor({ page: 'components', css: '[data-testid="components-page"]' });

  for (const [testId, destination] of destinations) {
    await app.page.click({ page: 'components', css: `[data-testid="${testId}"]` });
    await waitForCurrentPage(app, destination, 30_000);
    expect((await app.nav.current()).name).toBe(destination);

    await app.nav.back();
    await waitForCurrentPage(app, 'components', 30_000);
    await app.page.waitFor({ page: 'components', css: `[data-testid="${testId}"]` });
  }
});
