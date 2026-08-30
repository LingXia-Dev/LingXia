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
  const wrapped = await eventually(
    () => app.page.eval({
      page: 'video',
      script:
        '(() => { const root = document.querySelector("#video-native-root"); const video = root && root.querySelector(":scope > lx-video"); const compiled = root && typeof root.lastCompileResult === "function" ? root.lastCompileResult() : null; const children = compiled && compiled.ok ? compiled.root.children : []; return { hasRoot: !!root, videoIsDirectChild: !!video, videoId: video && video.getAttribute("id"), compileOk: !!(compiled && compiled.ok), kinds: children.map((child) => child.kind), hasCover: children.some((child) => child.authorType === "LxNativeCover") }; })()',
    }),
    (value) => (value as { compileOk?: boolean; kinds?: string[] } | null)?.compileOk === true
      && (value as { kinds: string[] }).kinds.join(',') === 'video',
    { timeoutMs: 5_000, describe: 'native video without the default overlay' },
  ) as {
    hasRoot: boolean;
    videoIsDirectChild: boolean;
    videoId: string | null;
    compileOk: boolean;
    kinds: string[];
    hasCover: boolean;
  };
  expect(wrapped.hasRoot).toBeTruthy();
  expect(wrapped.videoIsDirectChild).toBeTruthy();
  expect(wrapped.videoId).toBe('lx-video-1');
  expect(wrapped.compileOk).toBeTruthy();
  expect(wrapped.kinds.join(',')).toBe('video');
  expect(wrapped.hasCover).toBeFalsy();
  expect(await waitForElementText(
    app,
    'video',
    '[data-testid="native-menu-state"]',
    (text) => text === 'closed',
    5_000,
  )).toBe('closed');

  await app.page.click({ page: 'video', css: '[data-testid="native-menu-toggle"]' });
  expect(await waitForElementText(
    app,
    'video',
    '[data-testid="native-menu-state"]',
    (text) => text === 'open',
    5_000,
  )).toBe('open');
  const nativeMenu = await eventually(
    () => app.page.eval({
      page: 'video',
      script:
        '(() => { const root = document.querySelector("#video-native-root"); const compiled = root && typeof root.lastCompileResult === "function" ? root.lastCompileResult() : null; const children = compiled && compiled.ok ? compiled.root.children : []; const cover = children.find((child) => child.authorType === "LxNativeCover"); const menu = cover && cover.children.find((child) => child.authorId === "video-native-menu"); const more = menu && menu.children.find((child) => child.authorId === "video-native-menu-more"); const close = menu && menu.children.find((child) => child.authorId === "video-native-menu-close"); return { compileOk: !!(compiled && compiled.ok), kinds: children.map((child) => child.kind), cover: cover && { authorType: cover.authorType, automationId: cover.automationId, pointerEvents: cover.props.pointerEvents, scrim: cover.props.scrimPaint && cover.props.scrimPaint.scrim, coverPosition: cover.props.coverPreset && cover.props.coverPreset.position, coverInset: cover.props.coverPreset && cover.props.coverPreset.inset, childKinds: cover.children.map((child) => child.kind) }, menu: menu && { authorType: menu.authorType, automationId: menu.automationId, pointerEvents: menu.props.pointerEvents, nativeStyle: menu.props.nativeStyle, childKinds: menu.children.map((child) => child.kind), childText: menu.children.filter((child) => child.kind === "text").map((child) => child.text) }, more: more && { icon: more.props.content && more.props.content.icon && more.props.content.icon.name, label: more.props.content && more.props.content.text }, close: close && { icon: close.props.content && close.props.content.icon && close.props.content.icon.name, label: close.props.content && close.props.content.text } }; })()',
    }),
    (value) => (value as { menu?: { authorType?: unknown } } | null)?.menu?.authorType === 'LxNativeView',
    { timeoutMs: 5_000, describe: 'H5 burger mounted the native menu view' },
  ) as {
    compileOk: boolean;
    kinds: string[];
    cover: {
      authorType: string;
      automationId: string;
      pointerEvents: string;
      scrim: string;
      coverPosition: string;
      coverInset: number;
      childKinds: string[];
    };
    menu: {
      authorType: string;
      automationId: string;
      pointerEvents: string;
      nativeStyle: Record<string, string>;
      childKinds: string[];
      childText: string[];
    };
    more: { icon: string; label: string };
    close: { icon: string; label: string };
  };
  expect(nativeMenu.compileOk).toBeTruthy();
  expect(nativeMenu.kinds.join(',')).toBe('video,view');
  expect(nativeMenu.cover.authorType).toBe('LxNativeCover');
  expect(nativeMenu.cover.automationId).toBe('video-native-cover');
  expect(nativeMenu.cover.pointerEvents).toBe('box-none');
  expect(nativeMenu.cover.scrim).toBe('none');
  expect(nativeMenu.cover.coverPosition).toBe('absolute');
  expect(nativeMenu.cover.coverInset).toBe(0);
  expect(nativeMenu.cover.childKinds.join(',')).toBe('view');
  expect(nativeMenu.menu.authorType).toBe('LxNativeView');
  expect(nativeMenu.menu.automationId).toBe('video-native-menu');
  expect(nativeMenu.menu.pointerEvents).toBe('auto');
  expect(nativeMenu.menu.nativeStyle.backgroundColor).toContain('15');
  expect(nativeMenu.menu.nativeStyle.borderColor).toContain('71');
  expect(nativeMenu.menu.nativeStyle.borderRadius).toBe('12px');
  expect(nativeMenu.menu.childKinds.join(',')).toBe('text,text,tappable,tappable');
  expect(nativeMenu.menu.childText.join(' ')).toContain('NativeView above native video');
  expect(nativeMenu.more.icon).toBe('more');
  expect(nativeMenu.more.label).toBe('More');
  expect(nativeMenu.close.icon).toBe('close');
  expect(nativeMenu.close.label).toBe('Close');

  await app.page.click({ page: 'video', css: '[data-testid="native-menu-toggle"]' });
  expect(await waitForElementText(
    app,
    'video',
    '[data-testid="native-menu-state"]',
    (text) => text === 'closed',
    5_000,
  )).toBe('closed');
  const menuRemoved = await eventually(
    () => app.page.eval({
      page: 'video',
      script:
        '(() => { const root = document.querySelector("#video-native-root"); const compiled = root && typeof root.lastCompileResult === "function" ? root.lastCompileResult() : null; const children = compiled && compiled.ok ? compiled.root.children : []; return { compileOk: !!(compiled && compiled.ok), kinds: children.map((child) => child.kind), hasMenu: children.some((child) => child.authorType === "LxNativeCover") }; })()',
    }),
    (value) => (value as { compileOk?: boolean; hasMenu?: boolean } | null)?.compileOk === true
      && (value as { hasMenu: boolean }).hasMenu === false,
    { timeoutMs: 5_000, describe: 'native menu removed from compiled island' },
  ) as { compileOk: boolean; kinds: string[]; hasMenu: boolean };
  expect(menuRemoved.kinds.join(',')).toBe('video');

  const chrome = await app.page.eval({
    page: 'video',
    script:
      '(() => { const root = document.querySelector("#island-controls"); const compiled = root && typeof root.lastCompileResult === "function" ? root.lastCompileResult() : null; const kids = compiled && compiled.ok ? compiled.root.children : []; const kinds = []; const flat = []; const walk = (nodes) => { for (const node of nodes || []) { kinds.push(node.kind); flat.push(node); walk(node.children); } }; walk(kids); const view = flat.find((node) => node.authorId === "island-controls-view"); const status = flat.find((node) => node.authorId === "island-controls-status"); const button = flat.find((node) => node.authorId === "island-play"); const slider = flat.find((node) => node.authorId === "island-seek"); return { compileOk: !!(compiled && compiled.ok), kinds, hasPlay: !!document.querySelector("#island-play"), hasSeek: !!document.querySelector("#island-seek"), view: view && { authorType: view.authorType, automationId: view.automationId, pointerEvents: view.props.pointerEvents, nativeStyle: view.props.nativeStyle, childKinds: view.children.map((child) => child.kind) }, statusText: status && status.text, button: button && { automationId: button.automationId, intent: button.props.intent, emphasis: button.props.emphasis, hitSlop: button.props.hitSlop, ariaLabel: button.props["aria-label"], ariaDescription: button.props["aria-description"], nativeStyle: button.props.nativeStyle }, slider: slider && { value: slider.props.value, step: slider.props.step, bufferedValue: slider.props.bufferedValue, valueLabel: slider.props.valueLabel, nativeStyle: slider.props.nativeStyle } }; })()',
  }) as {
    compileOk: boolean;
    kinds: string[];
    hasPlay: boolean;
    hasSeek: boolean;
    view: {
      authorType: string;
      automationId: string;
      pointerEvents: string;
      nativeStyle: Record<string, string>;
      childKinds: string[];
    };
    statusText: string;
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
  expect(chrome.view.authorType).toBe('LxNativeView');
  expect(chrome.view.automationId).toBe('island-controls-view');
  expect(chrome.view.pointerEvents).toBe('auto');
  expect(chrome.view.nativeStyle.backgroundColor).toContain('15');
  expect(chrome.view.nativeStyle.borderColor).toContain('51');
  expect(chrome.view.nativeStyle.borderWidth).toBe('1px');
  expect(chrome.view.nativeStyle.borderRadius).toBe('14px');
  expect(chrome.view.childKinds.join(',')).toBe('text,text,tappable,slider');
  expect(chrome.statusText).toBe('waiting for native input');
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
  expect(chrome.slider.nativeStyle.accentColor).toContain('59');
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
