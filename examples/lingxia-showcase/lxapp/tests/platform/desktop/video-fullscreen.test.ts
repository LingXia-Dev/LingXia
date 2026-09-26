import { expect, spec } from '@lingxia/test';
import type { DesktopWindowInfo } from '@lingxia/types/automation';
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

function showcaseHost(windows: DesktopWindowInfo[]): DesktopWindowInfo | undefined {
  return windows
    .filter((window) => {
      const title = window.title.toLocaleLowerCase();
      const process = window.process.toLocaleLowerCase();
      return window.visible
        && window.bounds.w >= 320
        && window.bounds.h >= 320
        && (title.includes('lingxia') || process.includes('lingxiademo'));
    })
    .sort((left, right) => (
      right.bounds.w * right.bounds.h - left.bounds.w * left.bounds.h
    ))[0];
}

fullscreenSpec('enter and leave native video fullscreen from VideoContext', {
  id: 'DESKTOP-VIDEO-FULLSCREEN-001',
  covers: ['lx.createVideoContext', 'VideoContext.requestFullScreen', 'VideoContext.exitFullScreen', 'DesktopDriver.windows', 'DesktopDriver.displays'],
  app: SHOWCASE_APP_ID,
  timeout: 60_000,
  reason: 'needs the HTTP fixture: node tests/harness/http-fixture.mjs, then --arg httpBase=<url>',
}, async (t) => {
  const { app, defer } = bindFixture(t, 'DESKTOP-VIDEO-FULLSCREEN-001');
  const desktop = t.automation.desktop;
  type VideoCommand = 'play' | 'stop' | 'requestFullScreen' | 'exitFullScreen';
  const command = (action: VideoCommand) => app.logic.eval(({ lx }, id, action) => {
    const context = lx.createVideoContext(id);
    if (action === 'play') context.play();
    else if (action === 'stop') context.stop();
    else if (action === 'requestFullScreen') context.requestFullScreen();
    else context.exitFullScreen();
    return true;
  }, VIDEO_ID, action);
  // Native fullscreen is a separate window that takes a whole display; the
  // host's main window keeps its size. So the physical proof is that window.
  const displays = await desktop.displays();
  const coversADisplay = (window: DesktopWindowInfo): boolean => displays.some((display) => (
    window.bounds.w >= display.bounds.w && window.bounds.h >= display.bounds.h - 40
  ));
  const fullscreenWindows = async (): Promise<DesktopWindowInfo[]> => (await desktop.windows())
    .filter((window) => window.visible && coversADisplay(window));
  const eventLog = () => app.view.testId('video-event', { page: 'video' }).query()
    .then((element) => (element.exists ? element.text : ''));

  const current = await currentPageOrNull(app);
  if (current?.name !== 'home') await app.nav.relaunch({ page: 'home' });
  await waitForCurrentPageVisible(app, 'home', '[data-testid="home-page"]');
  defer(async () => {
    await command('exitFullScreen').catch(() => undefined);
    await command('stop').catch(() => undefined);
    const active = await currentPageOrNull(app);
    if (active?.name !== 'home') await app.nav.relaunch({ page: 'home' });
    await waitForCurrentPageVisible(app, 'home', '[data-testid="home-page"]');
  });

  // Desktop chrome cases before this one can leave a split, a surface float,
  // or a rail hover child window over the page. AVPlayer then accepts play()
  // without ever emitting Playing — the same leftover-panel class the entry
  // order comment on preview-https is guarding.
  const layout = await app.surfaceLayout();
  for (const float of layout.floats) {
    await app.logic.eval({ timeout: 15_000 }, async ({ lx }, key) => {
      const handle = lx.surface.getByKey(key);
      if (handle && 'close' in handle) await handle.close();
    }, float.id).catch(() => undefined);
  }
  const terminalVisible = layout.asides.some((surface) => surface.id === 'terminal')
    || layout.asideSlots.some((slot) => (
      slot.children.includes('terminal') && slot.visible && !slot.overlay
    ));
  if (terminalVisible) {
    await app.logic.eval({ timeout: 20_000 }, async ({ lx }) => {
      const handle = await lx.surface.openDeclared('terminal');
      if (handle.alive) await handle.close();
    }).catch(() => undefined);
  }
  const host = showcaseHost(await desktop.windows());
  if (host) {
    await desktop.window.focus({ window: host.id });
    await desktop.pointer.move({
      at: [
        Math.round(host.bounds.x + host.bounds.w * 0.62),
        Math.round(host.bounds.y + host.bounds.h * 0.55),
      ],
    });
  }

  await app.nav.to({
    page: 'video',
    query: { automationFixture: 'video-source', src: `${httpBase}/media/sample.mp4` },
  });
  await waitForCurrentPage(app, 'video');
  await app.view.testId('video-page', { page: 'video' }).waitFor({ state: 'visible', timeout: 30_000 });
  await app.view.css(`#${VIDEO_ID}`, { page: 'video' }).first().waitFor({ state: 'visible', timeout: 30_000 });
  await eventually(async () => {
    await command('play');
    return eventLog();
  }, (text) => text.includes('Playing'), {
    describe: 'video [data-testid="video-event"] text',
    timeoutMs: 15_000,
    intervalMs: 400,
  });
  const before = new Set((await fullscreenWindows()).map((window) => window.id));

  await t.step('requestFullScreen() reports on and presents a display-sized window', async () => {
    await command('requestFullScreen');
    await waitForElementText(t, 'video', '[data-testid="video-event"]', (text) => text.includes('Fullscreen: on'), 10_000);
    // The physical proof: a window really took a display, not just the event.
    const presented = await eventually(
      async () => (await fullscreenWindows()).filter((window) => !before.has(window.id)),
      (windows) => windows.length > 0,
      { describe: 'a display-sized fullscreen window to appear', timeoutMs: 10_000 },
    );
    expect(presented.length).toBeGreaterThan(0);
  });

  await t.step('exitFullScreen() reports off and takes that window down', async () => {
    await command('exitFullScreen');
    await waitForElementText(t, 'video', '[data-testid="video-event"]', (text) => text.includes('Fullscreen: off'), 10_000);
    await eventually(
      async () => (await fullscreenWindows()).filter((window) => !before.has(window.id)),
      (windows) => windows.length === 0,
      { describe: 'the fullscreen window to disappear', timeoutMs: 10_000 },
    );
    expect(await eventLog()).toContain('Fullscreen: off');
  });
});
