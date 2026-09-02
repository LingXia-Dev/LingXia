import { expect, spec } from '@lingxia/test';
import type { DesktopWindowInfo } from 'lingxia-types/automation';
import { SHOWCASE_APP_ID } from '../../helpers/app.js';
import { currentPageOrNull, waitForCurrentPage, waitForCurrentPageVisible, waitForElementText } from '../../helpers/page.js';
import { bindFixture, eventually } from '../../helpers/poll.js';

const testArgs = globalThis.__LINGXIA_AUTOMATION_HOST__?.args ?? {} as Record<string, string>;
const httpBase = testArgs.httpBase;
const selectedGate = testArgs.gate?.toLocaleLowerCase();
// Windows rejects `requestFullScreen` with an internal error, so the contract
// is proven where the host implements it; PEND-VIDEO-FS-WINDOWS-001 owns the gap.
const targetPlatform = testArgs.platform?.toLowerCase();
const fullscreenSpec = httpBase && !selectedGate && targetPlatform !== 'windows' ? spec : spec.skip;
const VIDEO_ID = 'lx-video-source-fixture';

fullscreenSpec('enter and leave native video fullscreen from VideoContext', {
  id: 'DESKTOP-VIDEO-FULLSCREEN-001',
  covers: ['lx.createVideoContext', 'VideoContext.requestFullScreen', 'VideoContext.exitFullScreen', 'DesktopDriver.windows', 'DesktopDriver.displays'],
  app: SHOWCASE_APP_ID,
  timeout: 60_000,
  reason: 'needs the HTTP fixture: node tests/harness/http-fixture.mjs, then --arg httpBase=<url>',
}, async (t) => {
  const { app, defer } = bindFixture(t, 'DESKTOP-VIDEO-FULLSCREEN-001');
  const desktop = lx.automation().desktop;
  const command = (body: string) => app.eval({
    script: `lx.createVideoContext(${JSON.stringify(VIDEO_ID)}).${body}; return true;`,
  });
  // Native fullscreen is a separate window that takes a whole display; the
  // host's main window keeps its size. So the physical proof is that window.
  const displays = await desktop.displays();
  const coversADisplay = (window: DesktopWindowInfo): boolean => displays.some((display) => (
    window.bounds.w >= display.bounds.w && window.bounds.h >= display.bounds.h - 40
  ));
  const fullscreenWindows = async (): Promise<DesktopWindowInfo[]> => (await desktop.windows())
    .filter((window) => window.visible && coversADisplay(window));
  const eventLog = () => app.page.query({ page: 'video', css: '[data-testid="video-event"]', full: true })
    .then((element) => (element.exists ? element.text : ''));

  const current = await currentPageOrNull(app);
  if (current?.name !== 'home') await app.nav.relaunch({ page: 'home' });
  await waitForCurrentPageVisible(app, 'home', '[data-testid="home-page"]');
  defer(async () => {
    await command('exitFullScreen()').catch(() => undefined);
    await command('stop()').catch(() => undefined);
    const active = await currentPageOrNull(app);
    if (active?.name !== 'home') await app.nav.relaunch({ page: 'home' });
    await waitForCurrentPageVisible(app, 'home', '[data-testid="home-page"]');
  });

  await app.nav.to({
    page: 'video',
    query: { automationFixture: 'video-source', src: `${httpBase}/media/sample.mp4` },
  });
  await waitForCurrentPage(app, 'video');
  await app.page.waitFor({ page: 'video', css: `#${VIDEO_ID}`, state: 'visible' });
  await command('play()');
  await waitForElementText(app, 'video', '[data-testid="video-event"]', (text) => text.includes('Playing'), 15_000);
  const before = new Set((await fullscreenWindows()).map((window) => window.id));

  await t.step('requestFullScreen() reports on and presents a display-sized window', async () => {
    await command('requestFullScreen()');
    await waitForElementText(app, 'video', '[data-testid="video-event"]', (text) => text.includes('Fullscreen: on'), 10_000);
    // The physical proof: a window really took a display, not just the event.
    const presented = await eventually(
      async () => (await fullscreenWindows()).filter((window) => !before.has(window.id)),
      (windows) => windows.length > 0,
      { describe: 'a display-sized fullscreen window to appear', timeoutMs: 10_000 },
    );
    expect(presented.length).toBeGreaterThan(0);
  });

  await t.step('exitFullScreen() reports off and takes that window down', async () => {
    await command('exitFullScreen()');
    await waitForElementText(app, 'video', '[data-testid="video-event"]', (text) => text.includes('Fullscreen: off'), 10_000);
    await eventually(
      async () => (await fullscreenWindows()).filter((window) => !before.has(window.id)),
      (windows) => windows.length === 0,
      { describe: 'the fullscreen window to disappear', timeoutMs: 10_000 },
    );
    expect(await eventLog()).toContain('Fullscreen: off');
  });
});
