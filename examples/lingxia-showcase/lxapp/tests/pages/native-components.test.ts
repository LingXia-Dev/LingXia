import { expect, test } from '@rongjs/test';
import {
  currentPageOrNull,
  waitForCurrentPage,
  waitForCurrentPageVisible,
  waitForElementText,
} from '../helpers/page.js';
import { contract } from '../support/contract.js';

async function attachWindow(name: string): Promise<void> {
  if (!test.attach) return;
  const screenshot = await lx.automation().lxapps.screenshot();
  await test.attach(name, { mimeType: 'image/png', base64: screenshot.base64 });
}

contract({
  id: 'NATIVE-VIDEO-001',
  title: 'hide the native video overlay before the next page becomes interactive',
  covers: ['lx.createVideoContext', 'VideoContext.pause', 'NavDriver.to', 'NavDriver.back'],
  layer: 'native',
  levels: ['semantic', 'boundary', 'lifecycle'],
  scope: 'portable',
  expectedOutcome: 'supported',
}, async ({ app, namespace, defer }) => {
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
  await attachWindow('native-video-active.png');

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
  await attachWindow('native-video-hidden.png');
});
