import { expect, spec, type Fixture } from '@lingxia/test';
import {
  currentPageOrNull,
  waitForCurrentPage,
  waitForCurrentPageVisible,
  waitForElementText,
} from '../helpers/page.js';
import { attachShot, bindFixture } from '../helpers/poll.js';
import { SHOWCASE_APP_ID } from '../helpers/app.js';

async function attachWindow(t: Fixture, name: string): Promise<void> {
  const screenshot = await lx.automation().lxapps.screenshot();
  await attachShot(t, name, { mimeType: 'image/png', base64: screenshot.base64 });
}

spec("hide the native video overlay before the next page becomes interactive", { id: "NATIVE-VIDEO-001", covers: ['lx.createVideoContext', 'VideoContext.pause', 'NavDriver.to', 'NavDriver.back'], app: SHOWCASE_APP_ID }, async (t) => {
  const { app, namespace, defer } = bindFixture(t, "NATIVE-VIDEO-001");

  const current = await currentPageOrNull(app);
  if (current?.name !== 'home') await app.nav.relaunch({ page: 'home' });
  await waitForCurrentPageVisible(app, 'home', '[data-testid="home-page"]');
  defer(async () => {
    const active = await currentPageOrNull(app);
    if (active?.name !== 'home') await app.nav.relaunch({ page: 'home' });
    await waitForCurrentPageVisible(app, 'home', '[data-testid="home-page"]');
  });

  await app.nav.to({
    page: 'video',
    query: { automationFixture: 'video-context-shape' },
  });
  await waitForCurrentPage(app, 'video');
  await app.page.waitFor({ page: 'video', css: '[data-testid="video-page"]', state: 'visible' });
  await app.page.waitFor({ page: 'video', css: '#lx-video-shape-fixture', state: 'visible' });
  // The shape fixture loads no media, and only Apple emits a pause event
  // without a playing transition; just exercise the pause command itself.
  await app.page.click({ page: 'video', css: '[data-testid="video-pause"]' });
  await attachWindow(t, 'native-video-active.png');

  const hiddenAt = Date.now();
  await app.nav.back();
  await waitForCurrentPageVisible(app, 'home', '[data-testid="home-page"]', 5_000);

  const name = `Native overlay ${namespace}`;
  await app.page.fill({ page: 'home', css: '[data-testid="home-name"]', text: name });
  await app.page.click({ page: 'home', css: '[data-testid="home-greet"]' });
  expect(await waitForElementText(
    app,
    'home',
    '[data-testid="home-greeting"]',
    (text) => text.includes(name),
    5_000,
  )).toContain(name);
  expect(Date.now() - hiddenAt < 5_000).toBeTruthy();
  await attachWindow(t, 'native-video-hidden.png');
});
