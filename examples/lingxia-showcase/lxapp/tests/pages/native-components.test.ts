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

function isWindowsNativeAccent(pixel: { r: number; g: number; b: number }): boolean {
  return pixel.b >= 180 && pixel.b >= pixel.r + 80 && pixel.b >= pixel.g + 60;
}

spec("hand an H5 menu press to a native menu above the island video", { id: "NATIVE-ISLAND-001", covers: ['lx.createVideoContext', 'NavDriver.to'], app: SHOWCASE_APP_ID, timeout: 30_000 }, async (t) => {
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
  expect(await waitForElementText(
    app,
    'video',
    '[data-testid="native-menu-js-result"]',
    (text) => text.includes('Tap Menu'),
    5_000,
  )).toContain('Tap Menu');

  await app.page.click({ page: 'video', css: '[data-testid="native-menu-toggle"]' });
  expect(await waitForElementText(
    app,
    'video',
    '[data-testid="native-menu-state"]',
    (text) => text === 'open',
    5_000,
  )).toBe('open');
  expect(await waitForElementText(
    app,
    'video',
    '[data-testid="native-menu-js-result"]',
    (text) => text === 'H5 mounted the native menu.',
    5_000,
  )).toBe('H5 mounted the native menu.');
  const nativeMenu = await eventually(
    () => app.page.eval({
      page: 'video',
      script:
        '(() => { const root = document.querySelector("#video-native-root"); const compiled = root && typeof root.lastCompileResult === "function" ? root.lastCompileResult() : null; const children = compiled && compiled.ok ? compiled.root.children : []; const cover = children.find((child) => child.authorType === "LxNativeCover"); const menu = cover && cover.children.find((child) => child.authorId === "video-native-menu"); const more = menu && menu.children.find((child) => child.authorId === "video-native-menu-more"); const close = menu && menu.children.find((child) => child.authorId === "video-native-menu-close"); return { compileOk: !!(compiled && compiled.ok), kinds: children.map((child) => child.kind), cover: cover && { authorType: cover.authorType, automationId: cover.automationId, pointerEvents: cover.props.pointerEvents, scrim: cover.props.scrimPaint && cover.props.scrimPaint.scrim, coverPosition: cover.props.coverPreset && cover.props.coverPreset.position, coverInset: cover.props.coverPreset && cover.props.coverPreset.inset, childKinds: cover.children.map((child) => child.kind) }, menu: menu && { authorType: menu.authorType, automationId: menu.automationId, role: menu.props.role, pointerEvents: menu.props.pointerEvents, nativeStyle: menu.props.nativeStyle, childKinds: menu.children.map((child) => child.kind), childText: menu.children.filter((child) => child.kind === "text").map((child) => child.text) }, more: more && { icon: more.props.content && more.props.content.icon && more.props.content.icon.name, label: more.props.content && more.props.content.text, intent: more.props.intent, emphasis: more.props.emphasis, nativeStyle: more.props.nativeStyle }, close: close && { icon: close.props.content && close.props.content.icon && close.props.content.icon.name, label: close.props.content && close.props.content.text, emphasis: close.props.emphasis, nativeStyle: close.props.nativeStyle } }; })()',
    }),
    (value) => (value as { menu?: { authorType?: unknown } } | null)?.menu?.authorType === 'LxNativeView',
      { timeoutMs: 5_000, describe: 'H5 menu trigger mounted the native menu view' },
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
      role: string;
      pointerEvents: string;
      nativeStyle: Record<string, string>;
      childKinds: string[];
      childText: string[];
    };
    more: { icon: string; label: string; intent: string; emphasis: string; nativeStyle: Record<string, string> };
    close: { icon: string; label: string; emphasis: string; nativeStyle: Record<string, string> };
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
  expect(nativeMenu.menu.role).toBe('menu');
  expect(nativeMenu.menu.pointerEvents).toBe('auto');
  expect(nativeMenu.menu.nativeStyle.backgroundColor).toContain('15');
  expect(nativeMenu.menu.nativeStyle.borderColor).toContain('100');
  expect(nativeMenu.menu.nativeStyle.borderRadius).toBe('14px');
  expect(nativeMenu.menu.childKinds.join(',')).toBe('text,text,tappable,tappable');
  expect(nativeMenu.menu.childText.join(' ')).toContain('NativeView above native video');
  expect(nativeMenu.more.icon).toBe('more');
  expect(nativeMenu.more.label).toBe('More');
  expect(nativeMenu.more.intent).toBe('accent');
  expect(nativeMenu.more.emphasis).toBe('primary');
  expect(nativeMenu.more.nativeStyle.borderRadius).toBe('10px');
  expect(nativeMenu.close.icon).toBe('close');
  expect(nativeMenu.close.label).toBe('Close');
  expect(nativeMenu.close.emphasis).toBe('secondary');
  expect(nativeMenu.close.nativeStyle.borderRadius).toBe('10px');

  const accessibleMenu = await app.page.eval({
    page: 'video',
    script: '(() => { const menu = document.querySelector("#video-native-menu"); const more = document.querySelector("#video-native-menu-more"); const close = document.querySelector("#video-native-menu-close"); const rect = menu?.getBoundingClientRect(); return { menuRole: menu?.getAttribute("role"), moreRole: more?.getAttribute("role"), closeRole: close?.getAttribute("role"), moreTabIndex: more?.getAttribute("tabindex"), closeTabIndex: close?.getAttribute("tabindex"), visible: !!rect && rect.top >= 0 && rect.left >= 0 && rect.bottom <= window.innerHeight && rect.right <= window.innerWidth }; })()',
  }) as { menuRole: string; moreRole: string; closeRole: string; moreTabIndex: string; closeTabIndex: string; visible: boolean };
  expect(accessibleMenu.menuRole).toBe('menu');
  expect(accessibleMenu.moreRole).toBe('button');
  expect(accessibleMenu.closeRole).toBe('button');
  expect(accessibleMenu.moreTabIndex).toBe('0');
  expect(accessibleMenu.closeTabIndex).toBe('0');
  expect(accessibleMenu.visible).toBeTruthy();

  const moreDispatched = await app.page.eval({
    page: 'video',
    script:
      '(() => { const more = document.querySelector("#video-native-menu-more"); if (!more) return false; more.dispatchEvent(new CustomEvent("press", { bubbles: true, detail: { source: "automation" } })); return true; })()',
  });
  expect(moreDispatched).toBeTruthy();
  expect(await waitForElementText(
    app,
    'video',
    '[data-testid="native-menu-state"]',
    (text) => text === 'closed',
    5_000,
  )).toBe('closed');
  expect(await waitForElementText(
    app,
    'video',
    '[data-testid="native-menu-js-result"]',
    (text) => text === 'More handled by View JS.',
    5_000,
  )).toBe('More handled by View JS.');
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

  await app.page.scrollTo({ page: 'video', css: '[data-testid="native-menu-toggle"]' });
  await app.page.click({ page: 'video', css: '[data-testid="native-menu-toggle"]' });
  const menuAfterScroll = await eventually(
    () => app.page.eval({
      page: 'video',
      script: '(() => { const menu = document.querySelector("#video-native-menu")?.getBoundingClientRect(); const toggle = document.querySelector("[data-testid=native-menu-toggle]")?.getBoundingClientRect(); return menu && toggle ? { menuTop: menu.top, menuBottom: menu.bottom, toggleBottom: toggle.bottom, viewportHeight: window.innerHeight } : null; })()',
    }),
    (value) => {
      const rect = value as { menuTop?: number; menuBottom?: number; toggleBottom?: number; viewportHeight?: number } | null;
      return rect != null
        && typeof rect.menuTop === 'number'
        && rect.menuTop >= 0
        && rect.menuBottom! <= rect.viewportHeight!
        && rect.menuTop >= rect.toggleBottom!;
    },
    { timeoutMs: 5_000, describe: 'native menu remains visible when opened after scrolling to its H5 trigger' },
  ) as { menuTop: number; menuBottom: number; toggleBottom: number; viewportHeight: number };
  expect(menuAfterScroll.menuTop >= menuAfterScroll.toggleBottom).toBeTruthy();
  const closeDispatched = await app.page.eval({
    page: 'video',
    script: '(() => { const close = document.querySelector("#video-native-menu-close"); if (!close) return false; close.dispatchEvent(new CustomEvent("press", { bubbles: true, detail: { source: "automation" } })); return true; })()',
  });
  expect(closeDispatched).toBeTruthy();
  expect(await waitForElementText(
    app,
    'video',
    '[data-testid="native-menu-state"]',
    (text) => text === 'closed',
    5_000,
  )).toBe('closed');

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
  await app.page.scrollTo({ page: 'video', css: '[data-testid="native-menu-toggle"]' });
  await app.page.click({ page: 'video', css: '[data-testid="native-menu-toggle"]' });
  let nativeButton = await eventually(
    () => app.page.query({ page: 'video', css: '#video-native-menu-more' }),
    (button) => button.exists && button.visible,
    { timeoutMs: 5_000, describe: 'native menu More button mounted over the video' },
  );
  if (!nativeButton.exists || !nativeButton.visible) {
    throw new Error('native menu More button was not visible after opening the H5 menu');
  }
  const beforeScrollCenterY = nativeButton.rect.center_y;
  const starveAnimationFrames = testArgs.platform?.toLocaleLowerCase() === 'windows';
  if (starveAnimationFrames) {
    const scrollY = await app.page.eval({
      page: 'video',
      script: 'globalThis.__nativeIslandScrollCompiles = 0; document.querySelector("#video-native-root")?.addEventListener("lxnativecompiled", () => { globalThis.__nativeIslandScrollCompiles += 1; }); globalThis.__nativeIslandOriginalRaf = window.requestAnimationFrame; globalThis.__nativeIslandDeferredRafs = []; window.requestAnimationFrame = (callback) => { globalThis.__nativeIslandDeferredRafs.push(callback); return 2147483647 + globalThis.__nativeIslandDeferredRafs.length; }; window.scrollTo(0, Math.min(80, document.documentElement.scrollHeight - window.innerHeight)); window.scrollY',
    });
    nativeButton = await eventually(
      async () => ({
        button: await app.page.query({ page: 'video', css: '#video-native-menu-more' }),
        compiles: await app.page.eval({ page: 'video', script: 'globalThis.__nativeIslandScrollCompiles' }),
      }),
      (value) => value.button.exists
        && value.button.visible
        && Math.abs(value.button.rect.center_y - beforeScrollCenterY) >= 40
        && typeof value.compiles === 'number'
        && value.compiles > 0,
      { timeoutMs: 5_000, describe: `native island geometry published without an animation frame (scrollY=${scrollY})` },
    ).then((value) => value.button);
    await app.page.eval({
      page: 'video',
      script: 'const deferred = globalThis.__nativeIslandDeferredRafs || []; window.requestAnimationFrame = globalThis.__nativeIslandOriginalRaf; delete globalThis.__nativeIslandOriginalRaf; delete globalThis.__nativeIslandDeferredRafs; deferred.forEach((callback) => callback(performance.now()));',
    });
  } else {
    await app.page.scroll({ page: 'video', dy: -80 });
    nativeButton = await eventually(
      () => app.page.query({ page: 'video', css: '#video-native-menu-more' }),
      (button) => button.exists
        && button.visible
        && Math.abs(button.rect.center_y - beforeScrollCenterY) >= 40,
      { timeoutMs: 5_000, describe: 'native island element followed cross-platform page scroll' },
    );
  }
  if (!nativeButton.exists) throw new Error('native menu More button disappeared after scroll');
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
    const accessibleMore = await eventually(
      async () => {
        try {
          return (await desktop.ax.query({
            window: host.id,
            match: 'More native menu actions',
            all: true,
          })).find((node) => node.enabled && node.role === 'button' && node.rect.w > 0 && node.rect.h > 0);
        } catch {
          return undefined;
        }
      },
      (value) => value !== undefined,
      { timeoutMs: 5_000, describe: 'native menu More action exposed as a Windows UIA button' },
    );
    if (!accessibleMore) throw new Error('native menu More action was absent from Windows UIA');
    const inputDiagnostic = `host ${JSON.stringify(host.bounds)}, UIA ${JSON.stringify(accessibleMore.rect)}`;
    await desktop.window.focus({ window: host.id });
    const visualProbePoints = [
      [0.2, 0.25],
      [0.5, 0.2],
      [0.8, 0.25],
      [0.2, 0.75],
      [0.8, 0.75],
    ].map(([x, y]): [number, number] => [
      Math.round(accessibleMore.rect.x + accessibleMore.rect.w * x),
      Math.round(accessibleMore.rect.y + accessibleMore.rect.h * y),
    ]);
    const accentSamples = await eventually(
      async () => {
        const pixels = await Promise.all(visualProbePoints.map((at) => desktop.pixel({ at })));
        return pixels.filter(isWindowsNativeAccent).length;
      },
      (count) => count >= 4,
      { timeoutMs: 5_000, describe: `native island visual followed scroll (${inputDiagnostic})` },
    );
    expect(accentSamples >= 4).toBeTruthy();
    nativeButton = await app.page.query({ page: 'video', css: '#video-native-menu-more' });
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
    expect(await waitForElementText(
      app,
      'video',
      '[data-testid="native-menu-js-result"]',
      (text) => text === 'More handled by View JS.',
      5_000,
    )).toBe('More handled by View JS.');

    await app.page.scrollTo({ page: 'video', css: '[data-testid="native-menu-toggle"]' });
    await app.page.click({ page: 'video', css: '[data-testid="native-menu-toggle"]' });
    await eventually(
      () => app.page.query({ page: 'video', css: '#video-native-menu-more' }),
      (button) => button.exists && button.visible,
      { timeoutMs: 5_000, describe: 'native menu remounted for keyboard activation' },
    );
    await desktop.ax.focus({ window: host.id, match: 'name:More native menu actions' });
    await eventually(
      () => app.page.eval({ page: 'video', script: 'document.activeElement?.id' }),
      (value) => value === 'video-native-menu-more',
      { timeoutMs: 5_000, describe: 'Windows UIA focus reached the native menu More element' },
    );
    await app.page.eval({
      page: 'video',
      script: 'document.activeElement?.dispatchEvent(new KeyboardEvent("keydown", { key: "Enter", bubbles: true, cancelable: true }))',
    });
    expect(await waitForElementText(
      app,
      'video',
      '[data-testid="native-press-source"]',
      (text) => text === 'keyboard',
      5_000,
    )).toBe('keyboard');
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
