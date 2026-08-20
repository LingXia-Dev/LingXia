import { expect, spec, type Fixture } from '@lingxia/test';
import {
  currentPageOrNull,
  waitForCurrentPage,
  waitForCurrentPageVisible,
  waitForElementText,
} from '../helpers/page.js';
import { attachShot, bindFixture, eventually } from '../helpers/poll.js';
import { SHOWCASE_APP_ID } from '../helpers/app.js';

const testGlobals = globalThis as typeof globalThis & {
  __LINGXIA_TEST__?: { run: () => Promise<unknown> };
  __RONG_TEST__?: { run: () => Promise<unknown> };
};
if (testGlobals.__LINGXIA_TEST__ && !testGlobals.__RONG_TEST__) {
  testGlobals.__RONG_TEST__ = testGlobals.__LINGXIA_TEST__;
}

async function attachWindow(t: Fixture, name: string): Promise<void> {
  const screenshot = await lx.automation().lxapps.screenshot();
  await attachShot(t, name, { mimeType: 'image/png', base64: screenshot.base64 });
}

spec("wrap the showcase player in LxNativeRoot so island video is the live path", { id: "NATIVE-ISLAND-001", covers: ['lx.createVideoContext', 'NavDriver.to'], app: SHOWCASE_APP_ID, timeout: 30_000 }, async (t) => {
  const { app, defer } = bindFixture(t, "NATIVE-ISLAND-001");
  const current = await currentPageOrNull(app);
  if (current?.name !== 'home') await app.nav.relaunch({ page: 'home' });
  await waitForCurrentPageVisible(app, 'home', '[data-testid="home-page"]');
  defer(async () => {
    const active = await currentPageOrNull(app);
    if (active?.name !== 'home') await app.nav.relaunch({ page: 'home' });
  });

  await app.nav.relaunch({ page: 'video' });
  await waitForCurrentPage(app, 'video');
  await app.page.waitFor({ page: 'video', css: 'lx-native-root', state: 'attached' });
  const wrapped = await app.page.eval({
    page: 'video',
    script:
      '(() => { const root = document.querySelector("lx-native-root"); const video = document.querySelector("lx-native-root > lx-video"); const compiled = root && typeof root.lastCompileResult === "function" ? root.lastCompileResult() : null; return { hasRoot: !!root, videoIsDirectChild: !!video, videoId: video && video.getAttribute("id"), compileOk: !!(compiled && compiled.ok), kinds: compiled && compiled.ok ? compiled.root.children.map((child) => child.kind) : [] }; })()',
  });
  expect(wrapped.hasRoot).toBeTruthy();
  expect(wrapped.videoIsDirectChild).toBeTruthy();
  expect(wrapped.videoId).toBe('lx-video-1');
  expect(wrapped.compileOk).toBeTruthy();
  expect(wrapped.kinds[0]).toBe('video');
  expect(wrapped.kinds[1]).toBe('view');
  const chrome = await app.page.eval({
    page: 'video',
    script:
      '(() => { const root = document.querySelector("#island-controls"); const compiled = root && typeof root.lastCompileResult === "function" ? root.lastCompileResult() : null; const kids = compiled && compiled.ok ? compiled.root.children : []; const kinds = []; const walk = (nodes) => { for (const node of nodes || []) { kinds.push(node.kind); walk(node.children); } }; walk(kids); return { compileOk: !!(compiled && compiled.ok), kinds, hasPlay: !!document.querySelector("#island-play"), hasSeek: !!document.querySelector("#island-seek") }; })()',
  });
  expect(chrome.compileOk).toBeTruthy();
  expect(chrome.hasPlay).toBeTruthy();
  expect(chrome.hasSeek).toBeTruthy();
  expect(chrome.kinds.includes('tappable')).toBeTruthy();
  expect(chrome.kinds.includes('slider')).toBeTruthy();
  const playing = await eventually(
    () =>
      app.page.eval({
        page: 'video',
        script:
          'document.querySelector("lx-video") && document.querySelector("lx-video").getAttribute("data-lx-playing")',
      }),
    (value) => value === 'true',
    { timeoutMs: 20_000, describe: 'lx-video data-lx-playing' },
  );
  expect(playing).toBe('true');
  await attachShot(t, 'island-playing.png');
});

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
  expect(Date.now() - hiddenAt).toBeLessThan(5_000);
  await attachWindow(t, 'native-video-hidden.png');
});
