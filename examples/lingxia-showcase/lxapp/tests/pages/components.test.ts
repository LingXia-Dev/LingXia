import { expect, test } from '@rongjs/test';
import type { LxAppDriver } from 'lingxia-types';

async function waitForCurrent(app: LxAppDriver, page: string): Promise<void> {
  const deadline = Date.now() + 10_000;
  while (Date.now() < deadline) {
    const current = await app.nav.current();
    if (current.name === page && current.ready) return;
    await new Promise((resolve) => setTimeout(resolve, 50));
  }
  throw new Error(`Timed out waiting for component destination '${page}'`);
}

test('opens every component demo through rendered UI and the Logic bridge', async () => {
  const app = lx.automation().lxapp();
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
    await waitForCurrent(app, destination);
    expect((await app.nav.current()).name).toBe(destination);

    await app.nav.back();
    await waitForCurrent(app, 'components');
    await app.page.waitFor({ page: 'components', css: `[data-testid="${testId}"]` });
  }
});
