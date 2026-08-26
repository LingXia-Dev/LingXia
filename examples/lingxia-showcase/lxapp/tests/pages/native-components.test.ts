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
const testArgs = globalThis.__LINGXIA_AUTOMATION_HOST__?.args ?? {} as Record<string, string>;
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
  }) as {
    hasRoot: boolean;
    videoIsDirectChild: boolean;
    videoId: string | null;
    compileOk: boolean;
    kinds: string[];
  };
  expect(wrapped.hasRoot).toBeTruthy();
  expect(wrapped.videoIsDirectChild).toBeTruthy();
  expect(wrapped.videoId).toBe('lx-video-1');
  expect(wrapped.compileOk).toBeTruthy();
  expect(wrapped.kinds[0]).toBe('video');
  expect(wrapped.kinds[1]).toBe('view');
  const chrome = await app.page.eval({
    page: 'video',
    script:
      '(() => { const root = document.querySelector("#island-controls"); const compiled = root && typeof root.lastCompileResult === "function" ? root.lastCompileResult() : null; const kids = compiled && compiled.ok ? compiled.root.children : []; const kinds = []; const flat = []; const walk = (nodes) => { for (const node of nodes || []) { kinds.push(node.kind); flat.push(node); walk(node.children); } }; walk(kids); const button = flat.find((node) => node.authorId === "island-play"); const slider = flat.find((node) => node.authorId === "island-seek"); return { compileOk: !!(compiled && compiled.ok), kinds, hasPlay: !!document.querySelector("#island-play"), hasSeek: !!document.querySelector("#island-seek"), button: button && { automationId: button.automationId, intent: button.props.intent, emphasis: button.props.emphasis, hitSlop: button.props.hitSlop, ariaLabel: button.props["aria-label"], ariaDescription: button.props["aria-description"], nativeStyle: button.props.nativeStyle }, slider: slider && { value: slider.props.value, step: slider.props.step, bufferedValue: slider.props.bufferedValue, valueLabel: slider.props.valueLabel, nativeStyle: slider.props.nativeStyle } }; })()',
  }) as {
    compileOk: boolean;
    kinds: string[];
    hasPlay: boolean;
    hasSeek: boolean;
    button: {
      automationId: string;
      intent: string;
      emphasis: string;
      hitSlop: number;
      ariaLabel: string;
      ariaDescription: string;
      nativeStyle: Record<string, string>;
    };
    slider: {
      value: number;
      step: number;
      bufferedValue: number;
      valueLabel: string;
      nativeStyle: Record<string, string>;
    };
  };
  expect(chrome.compileOk).toBeTruthy();
  expect(chrome.hasPlay).toBeTruthy();
  expect(chrome.hasSeek).toBeTruthy();
  expect(chrome.kinds.includes('tappable')).toBeTruthy();
  expect(chrome.kinds.includes('slider')).toBeTruthy();
  expect(chrome.button.automationId).toBe('island-play-button');
  expect(chrome.button.intent).toBe('accent');
  expect(chrome.button.emphasis).toBe('primary');
  expect(chrome.button.hitSlop).toBe(8);
  expect(chrome.button.ariaLabel).toContain('island video');
  expect(chrome.button.ariaDescription).toBe('Controls the native video player');
  expect(chrome.button.nativeStyle.borderRadius).toBe('10px');
  expect(chrome.slider.step).toBe(1);
  expect(chrome.slider.bufferedValue >= chrome.slider.value).toBeTruthy();
  expect(chrome.slider.valueLabel).toBe('value');
  expect(chrome.slider.nativeStyle.accentColor).toContain('37');
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
  await app.page.scrollTo({ page: 'video', css: '#island-play' });
  const nativeButton = await app.page.query({ page: 'video', css: '#island-play' });
  if (!nativeButton.exists || !nativeButton.visible) {
    throw new Error('native island play button was not visible after scrollTo');
  }
  const automation = lx.automation();
  if (testArgs.platform?.toLocaleLowerCase() === 'windows') {
    const desktop = automation.desktop;
    const host = (await desktop.windows())
      .filter((window) => (
        window.visible
        && window.title === 'LingXia'
        && window.process.toLocaleLowerCase() !== 'msedgewebview2'
      ))
      .sort((left, right) => right.bounds.w * right.bounds.h - left.bounds.w * left.bounds.h)[0];
    if (!host) throw new Error('visible Windows showcase host window was not found');
    const accessibleButton = (await desktop.ax.query({
      window: host.id,
      match: 'island video',
      all: true,
    })).find((node) => node.enabled && node.rect.w > 0 && node.rect.h > 0);
    if (!accessibleButton) throw new Error('native island button was absent from Windows UIA');
    const inputDiagnostic = `host ${JSON.stringify(host.bounds)}, UIA ${JSON.stringify(accessibleButton.rect)}`;
    await automation.lxapp(SHOWCASE_APP_ID).page.pointer.click({
      window: host.id,
      at: [nativeButton.rect.center_x, nativeButton.rect.center_y],
    });
    const pressSource = await eventually(
      () => app.page.eval({
        page: 'video',
        script: 'document.querySelector("[data-testid=native-press-source]")?.textContent',
      }),
      (value) => value === 'pointer',
      { timeoutMs: 5_000, describe: `native button press source (${inputDiagnostic})` },
    );
    expect(pressSource).toBe('pointer');

    const nativeSlider = await app.page.query({ page: 'video', css: '#island-seek' });
    if (!nativeSlider.exists || !nativeSlider.visible) {
      throw new Error('native island slider was not visible after button interaction');
    }
    await app.page.eval({
      page: 'video',
      script: 'globalThis.__nativeSliderCommit = null; document.querySelector("#island-seek")?.addEventListener("valuecommit", (event) => { globalThis.__nativeSliderCommit = event.detail; }, { once: true });',
    });
    await automation.lxapp(SHOWCASE_APP_ID).page.pointer.drag({
      window: host.id,
      from: [nativeSlider.rect.left + nativeSlider.rect.width * 0.25, nativeSlider.rect.center_y],
      to: [nativeSlider.rect.left + nativeSlider.rect.width * 0.75, nativeSlider.rect.center_y],
    });
    const committed = await eventually(
      () => app.page.eval({ page: 'video', script: 'globalThis.__nativeSliderCommit' }),
      (value) => typeof (value as { value?: unknown } | null)?.value === 'number',
      { timeoutMs: 5_000, describe: 'native slider valuecommit' },
    ) as { value: number };
    expect(committed.value >= 70 && committed.value <= 80).toBeTruthy();
  }
  await attachWindow(t, 'island-playing.png');
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
