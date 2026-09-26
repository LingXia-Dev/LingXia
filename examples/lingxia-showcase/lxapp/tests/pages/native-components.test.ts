// Host e2e for inline native islands. JS protocol stays in @lingxia/elements Node tests.
import { expect, spec, type Fixture } from '@lingxia/test';
import {
  currentPageOrNull,
  waitForCurrentPage,
  waitForCurrentPageVisible,
  waitForElementText,
} from '../helpers/page.js';
import { attachShot, bindFixture, eventually } from '../helpers/poll.js';
import { SHOWCASE_APP_ID } from '../helpers/app.js';
import type { ProbeDocument, ProbeElement, ProbeWindow } from '../helpers/view.js';

/** A node of a native island's compiled tree, as its probes read it. */
interface NativeNode {
  kind: string;
  authorType?: string;
  authorId?: string;
  automationId?: string;
  text?: string;
  props: {
    pointerEvents?: string;
    scrimPaint?: { scrim?: string };
    coverPreset?: { position?: string; inset?: number };
    role?: string;
    nativeStyle?: Record<string, string>;
    content?: { icon?: { name?: string }; text?: string };
    intent?: string;
    emphasis?: string;
  };
  children: NativeNode[];
}

interface NativeCompileResult {
  ok: boolean;
  root: { children: NativeNode[] };
  diagnostics: Array<{ code?: string; message: string }>;
}

/** `<lx-native-root>` as the probes drive it. */
interface NativeRoot extends ProbeElement {
  lastCompileResult?: () => NativeCompileResult | null;
  compileNow(): NativeCompileResult;
}

/** Page globals the frame-starving probe parks on `window`. */
interface FrameProbeWindow extends ProbeWindow {
  __nativeIslandScrollCompiles?: number;
  __nativeIslandOriginalRaf?: ProbeWindow['requestAnimationFrame'];
  __nativeIslandDeferredRafs?: Array<(time: number) => void>;
  readonly performance: { now(): number };
  readonly KeyboardEvent: new (type: string, init: { key: string; bubbles?: boolean; cancelable?: boolean }) => unknown;
}

const testGlobals = globalThis as typeof globalThis & {
  __LINGXIA_TEST__?: { run: () => Promise<unknown> };
  __RONG_TEST__?: { run: () => Promise<unknown> };
};
const testArgs = globalThis.__LINGXIA_AUTOMATION_HOST__?.args ?? {} as Record<string, string>;
if (testGlobals.__LINGXIA_TEST__ && !testGlobals.__RONG_TEST__) {
  testGlobals.__RONG_TEST__ = testGlobals.__LINGXIA_TEST__;
}

async function attachWindow(t: Fixture, name: string): Promise<void> {
  const screenshot = await t.automation.lxapps.screenshot();
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
  await app.view.css('lx-native-root', { page: 'video' }).first().waitFor({ state: 'attached', timeout: 30_000 });
  const wrapped = await eventually(
    () => app.view.eval({ page: 'video' }, ({ document }) => {
      const root = document.querySelector('#video-native-root') as NativeRoot | null;
      const video = root && root.querySelector(':scope > lx-video');
      const compiled = root && typeof root.lastCompileResult === 'function' ? root.lastCompileResult() : null;
      const children = compiled && compiled.ok ? compiled.root.children : [];
      return {
        hasRoot: !!root,
        videoIsDirectChild: !!video,
        videoId: video && video.getAttribute('id'),
        compileOk: !!(compiled && compiled.ok),
        kinds: children.map((child) => child.kind),
        hasCover: children.some((child) => child.authorType === 'LxNativeCover'),
      };
    }),
    (value) => value?.compileOk === true && value.kinds.join(',') === 'video',
    { timeoutMs: 5_000, describe: 'native video without the default overlay' },
  );
  expect(wrapped.hasRoot).toBeTruthy();
  expect(wrapped.videoIsDirectChild).toBeTruthy();
  expect(wrapped.videoId).toBe('lx-video-1');
  expect(wrapped.compileOk).toBeTruthy();
  expect(wrapped.kinds.join(',')).toBe('video');
  expect(wrapped.hasCover).toBeFalsy();
  expect(await waitForElementText(
    t,
    'video',
    '[data-testid="native-menu-state"]',
    (text) => text === 'closed',
    5_000,
  )).toBe('closed');
  expect(await waitForElementText(
    t,
    'video',
    '[data-testid="native-menu-js-result"]',
    (text) => text.includes('Tap Menu'),
    5_000,
  )).toContain('Tap Menu');

  await app.view.testId("native-menu-toggle", { page: 'video' }).click();
  expect(await waitForElementText(
    t,
    'video',
    '[data-testid="native-menu-state"]',
    (text) => text === 'open',
    5_000,
  )).toBe('open');
  expect(await waitForElementText(
    t,
    'video',
    '[data-testid="native-menu-js-result"]',
    (text) => text === 'H5 mounted the native menu.',
    5_000,
  )).toBe('H5 mounted the native menu.');
  const nativeMenu = await eventually(
    () => app.view.eval({ page: 'video' }, ({ document }) => {
      const root = document.querySelector('#video-native-root') as NativeRoot | null;
      const compiled = root && typeof root.lastCompileResult === 'function' ? root.lastCompileResult() : null;
      const children = compiled && compiled.ok ? compiled.root.children : [];
      const cover = children.find((child) => child.authorType === 'LxNativeCover');
      const menu = cover && cover.children.find((child) => child.authorId === 'video-native-menu');
      const more = menu && menu.children.find((child) => child.authorId === 'video-native-menu-more');
      const close = menu && menu.children.find((child) => child.authorId === 'video-native-menu-close');
      return {
        compileOk: !!(compiled && compiled.ok),
        kinds: children.map((child) => child.kind),
        cover: cover && {
          authorType: cover.authorType,
          automationId: cover.automationId,
          pointerEvents: cover.props.pointerEvents,
          scrim: cover.props.scrimPaint && cover.props.scrimPaint.scrim,
          coverPosition: cover.props.coverPreset && cover.props.coverPreset.position,
          coverInset: cover.props.coverPreset && cover.props.coverPreset.inset,
          childKinds: cover.children.map((child) => child.kind),
        },
        menu: menu && {
          authorType: menu.authorType,
          automationId: menu.automationId,
          role: menu.props.role,
          pointerEvents: menu.props.pointerEvents,
          nativeStyle: menu.props.nativeStyle ?? {},
          childKinds: menu.children.map((child) => child.kind),
          childText: menu.children.filter((child) => child.kind === 'text').map((child) => child.text),
        },
        more: more && {
          icon: more.props.content && more.props.content.icon && more.props.content.icon.name,
          label: more.props.content && more.props.content.text,
          intent: more.props.intent,
          emphasis: more.props.emphasis,
          nativeStyle: more.props.nativeStyle ?? {},
        },
        close: close && {
          icon: close.props.content && close.props.content.icon && close.props.content.icon.name,
          label: close.props.content && close.props.content.text,
          emphasis: close.props.emphasis,
          nativeStyle: close.props.nativeStyle ?? {},
        },
      };
    }),
    (value) => value?.menu?.authorType === 'LxNativeView',
    { timeoutMs: 5_000, describe: 'H5 menu trigger mounted the native menu view' },
  );
  if (!nativeMenu.cover || !nativeMenu.menu || !nativeMenu.more || !nativeMenu.close) {
    throw new Error(`native menu tree is incomplete: ${JSON.stringify(nativeMenu)}`);
  }
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

  const accessibleMenu = await app.view.eval({ page: 'video' }, ({ document, window }) => {
    const menu = document.querySelector('#video-native-menu');
    const more = document.querySelector('#video-native-menu-more');
    const close = document.querySelector('#video-native-menu-close');
    const rect = menu?.getBoundingClientRect();
    return {
      menuRole: menu?.getAttribute('role'),
      moreRole: more?.getAttribute('role'),
      closeRole: close?.getAttribute('role'),
      moreAriaLabel: more?.getAttribute('aria-label'),
      closeAriaLabel: close?.getAttribute('aria-label'),
      moreTabIndex: more?.getAttribute('tabindex'),
      closeTabIndex: close?.getAttribute('tabindex'),
      visible: !!rect && rect.top >= 0 && rect.left >= 0
        && rect.top + rect.height <= window.innerHeight && rect.left + rect.width <= window.innerWidth,
    };
  });
  expect(accessibleMenu.menuRole).toBe('menu');
  expect(accessibleMenu.moreRole).toBe('button');
  expect(accessibleMenu.closeRole).toBe('button');
  expect(accessibleMenu.moreAriaLabel).toBe('More native menu actions');
  expect(accessibleMenu.closeAriaLabel).toBe('Close native menu');
  expect(accessibleMenu.moreTabIndex).toBe('0');
  expect(accessibleMenu.closeTabIndex).toBe('0');
  expect(accessibleMenu.visible).toBeTruthy();

  const contract = await app.view.eval({ page: 'video' }, ({ document }) => {
    const page = document as unknown as ProbeDocument;
    const root = document.querySelector('#video-native-root') as NativeRoot;
    const more = document.querySelector('#video-native-menu-more') as ProbeElement;
    const css = page.createElement('style');
    Object.assign(css, { textContent: '.native-contract-shadow { box-shadow: 0 2px 4px black; }' });
    page.head.appendChild(css);
    const errors: Array<{ code?: string; message: string }> = [];
    const onError = (event: unknown) => errors.push((event as { detail: { code?: string; message: string } }).detail);
    root.addEventListener('error', onError);
    try {
      more.setAttribute('icon', 'play');
      more.classList.add('native-contract-shadow');
      const first = root.compileNow();
      const count = errors.length;
      root.compileNow();
      const deduplicated = errors.length === count;
      more.classList.remove('native-contract-shadow');
      const restored = root.compileNow();
      return {
        combined: first.ok,
        diagnosed: errors.some((error) => error.code === 'NATIVE_ROOT_UNSUPPORTED_STYLE'
          && error.message.includes('video-native-menu-more') && error.message.includes('boxShadow')),
        deduplicated,
        recovered: restored.ok && !restored.diagnostics.some((error) => error.message.includes('boxShadow')),
      };
    } finally {
      more.setAttribute('icon', 'more');
      more.classList.remove('native-contract-shadow');
      css.remove();
      root.removeEventListener('error', onError);
      root.compileNow();
    }
  });
  expect(contract.combined).toBeTruthy();
  expect(contract.diagnosed).toBeTruthy();
  expect(contract.deduplicated).toBeTruthy();
  expect(contract.recovered).toBeTruthy();

  const moreDispatched = await app.view.eval({ page: 'video' }, ({ document, window }, id) => {
    const target = document.querySelector(id) as ProbeElement | null;
    if (!target) return false;
    const page = window as unknown as ProbeWindow;
    target.dispatchEvent(new page.CustomEvent('press', { bubbles: true, detail: { source: 'automation' } }));
    return true;
  }, '#video-native-menu-more');
  expect(moreDispatched).toBeTruthy();
  expect(await waitForElementText(
    t,
    'video',
    '[data-testid="native-menu-state"]',
    (text) => text === 'closed',
    5_000,
  )).toBe('closed');
  expect(await waitForElementText(
    t,
    'video',
    '[data-testid="native-menu-js-result"]',
    (text) => text === 'More handled by View JS.',
    5_000,
  )).toBe('More handled by View JS.');
  const menuRemoved = await eventually(
    () => app.view.eval({ page: 'video' }, ({ document }) => {
      const root = document.querySelector('#video-native-root') as NativeRoot | null;
      const compiled = root && typeof root.lastCompileResult === 'function' ? root.lastCompileResult() : null;
      const children = compiled && compiled.ok ? compiled.root.children : [];
      return {
        compileOk: !!(compiled && compiled.ok),
        kinds: children.map((child) => child.kind),
        hasMenu: children.some((child) => child.authorType === 'LxNativeCover'),
      };
    }),
    (value) => value?.compileOk === true && value.hasMenu === false,
    { timeoutMs: 5_000, describe: 'native menu removed from compiled island' },
  );
  expect(menuRemoved.kinds.join(',')).toBe('video');

  // The click scrolls the trigger into view first.
  await app.view.testId("native-menu-toggle", { page: 'video' }).click();
  const menuAfterScroll = await eventually(
    () => app.view.eval({ page: 'video' }, ({ document, window }) => {
      const menu = document.querySelector('#video-native-menu')?.getBoundingClientRect();
      const toggle = document.querySelector('[data-testid=native-menu-toggle]')?.getBoundingClientRect();
      return menu && toggle
        ? {
          menuTop: menu.top,
          menuBottom: menu.top + menu.height,
          toggleBottom: toggle.top + toggle.height,
          viewportHeight: window.innerHeight,
        }
        : null;
    }),
    (rect) => rect != null
      && rect.menuTop >= 0
      && rect.menuBottom <= rect.viewportHeight
      && rect.menuTop >= rect.toggleBottom,
    { timeoutMs: 5_000, describe: 'native menu remains visible when opened after scrolling to its H5 trigger' },
  );
  if (!menuAfterScroll) throw new Error('native menu geometry is missing');
  expect(menuAfterScroll.menuTop >= menuAfterScroll.toggleBottom).toBeTruthy();
  const closeDispatched = await app.view.eval({ page: 'video' }, ({ document, window }, id) => {
    const target = document.querySelector(id) as ProbeElement | null;
    if (!target) return false;
    const page = window as unknown as ProbeWindow;
    target.dispatchEvent(new page.CustomEvent('press', { bubbles: true, detail: { source: 'automation' } }));
    return true;
  }, '#video-native-menu-close');
  expect(closeDispatched).toBeTruthy();
  expect(await waitForElementText(
    t,
    'video',
    '[data-testid="native-menu-state"]',
    (text) => text === 'closed',
    5_000,
  )).toBe('closed');

  const playing = await eventually(
    () =>
      app.view.eval({ page: 'video' }, ({ document }) =>
        document.querySelector('lx-video')?.getAttribute('data-lx-playing') ?? null),
    (value) => value === 'true',
    { timeoutMs: 20_000, describe: 'lx-video data-lx-playing' },
  );
  expect(playing).toBe('true');
  await app.view.testId("native-menu-toggle", { page: 'video' }).click();
  const moreButton = app.view.css('#video-native-menu-more', { page: 'video' }).first();
  let nativeButton = await eventually(
    () => moreButton.query(),
    (button) => button.exists && button.visible && button.inViewport !== false,
    { timeoutMs: 5_000, describe: 'native menu More button mounted over the video' },
  );
  if (!nativeButton.exists || !nativeButton.visible) {
    throw new Error('native menu More button was not visible after opening the H5 menu');
  }
  const beforeScrollCenterY = nativeButton.rect.center_y;
  // A phone fits the whole video page in one screen, so there would be nothing
  // to scroll. Pad the document so geometry always has somewhere to follow.
  await app.view.eval({ page: 'video' }, ({ document }) => {
    const page = document as unknown as ProbeDocument;
    const spacer = page.createElement('div');
    spacer.setAttribute('id', 'native-island-scroll-spacer');
    spacer.style.height = '100vh';
    page.body.appendChild(spacer);
    return true;
  });
  defer(async () => {
    await app.view.eval({ page: 'video' }, ({ document, window }) => {
      (document.querySelector('#native-island-scroll-spacer') as ProbeElement | null)?.remove();
      (window as unknown as ProbeWindow).scrollTo(0, 0);
      return true;
    }).catch(() => {});
  });
  const starveAnimationFrames = testArgs.platform?.toLocaleLowerCase() === 'windows';
  if (starveAnimationFrames) {
    const scrollY = await app.view.eval({ page: 'video' }, ({ document, window }) => {
      const page = window as unknown as FrameProbeWindow;
      page.__nativeIslandScrollCompiles = 0;
      (document.querySelector('#video-native-root') as ProbeElement | null)?.addEventListener('lxnativecompiled', () => {
        page.__nativeIslandScrollCompiles = (page.__nativeIslandScrollCompiles ?? 0) + 1;
      });
      page.__nativeIslandOriginalRaf = page.requestAnimationFrame;
      const deferred: Array<(time: number) => void> = [];
      page.__nativeIslandDeferredRafs = deferred;
      page.requestAnimationFrame = (callback) => {
        deferred.push(callback);
        return 2147483647 + deferred.length;
      };
      page.scrollTo(0, Math.min(80, (document.documentElement as ProbeElement).scrollHeight - window.innerHeight));
      return window.scrollY;
    });
    nativeButton = await eventually(
      async () => ({
        button: await moreButton.query(),
        compiles: await app.view.eval({ page: 'video' }, ({ window }) =>
          (window as unknown as FrameProbeWindow).__nativeIslandScrollCompiles ?? null),
      }),
      (value) => value.button.exists
        && value.button.visible
        && value.button.inViewport !== false
        && Math.abs(value.button.rect.center_y - beforeScrollCenterY) >= 40
        && typeof value.compiles === 'number'
        && value.compiles > 0,
      { timeoutMs: 5_000, describe: `native island geometry published without an animation frame (scrollY=${scrollY})` },
    ).then((value) => value.button);
    await app.view.eval({ page: 'video' }, ({ window }) => {
      const page = window as unknown as FrameProbeWindow;
      const deferred = page.__nativeIslandDeferredRafs || [];
      if (page.__nativeIslandOriginalRaf) page.requestAnimationFrame = page.__nativeIslandOriginalRaf;
      delete page.__nativeIslandOriginalRaf;
      delete page.__nativeIslandDeferredRafs;
      deferred.forEach((callback) => callback(page.performance.now()));
      return true;
    });
  } else {
    // macOS `page.scroll` posts a wheel at the WebView center, which the
    // island consumes. Move the document itself so geometry has to follow.
    const scrollY = await app.view.eval({ page: 'video' }, ({ document, window }) => {
      const y = window.scrollY;
      const max = Math.max(0, (document.documentElement as ProbeElement).scrollHeight - window.innerHeight);
      const next = y + 80 <= max ? y + 80 : Math.max(0, y - 80);
      (window as unknown as ProbeWindow).scrollTo(0, next);
      return window.scrollY;
    });
    nativeButton = await eventually(
      () => moreButton.query(),
      (button) => button.exists
        && button.visible
        && button.inViewport !== false
        && Math.abs(button.rect.center_y - beforeScrollCenterY) >= 40,
      { timeoutMs: 5_000, describe: `native island element followed page scroll (scrollY=${scrollY})` },
    );
  }
  if (!nativeButton.exists) throw new Error('native menu More button disappeared after scroll');
  const automation = t.automation;
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
        } catch (error) {
          throw new Error(`Windows UIA query failed: ${String(error)}`);
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
    nativeButton = await moreButton.query();
    if (!nativeButton.exists) {
      throw new Error('native menu More button disappeared before pointer click');
    }
    await automation.lxapp(SHOWCASE_APP_ID).view.pointer.click({
      window: host.id,
      at: [nativeButton.rect.center_x, nativeButton.rect.center_y],
    });
    const pressSource = await eventually(
      () => app.view.eval({ page: 'video' }, ({ document }) =>
        document.querySelector('[data-testid=native-press-source]')?.textContent ?? null),
      (value) => value === 'pointer',
      { timeoutMs: 5_000, describe: `native button press source (${inputDiagnostic})` },
    );
    expect(pressSource).toBe('pointer');
    expect(await waitForElementText(
      t,
      'video',
      '[data-testid="native-menu-js-result"]',
      (text) => text === 'More handled by View JS.',
      5_000,
    )).toBe('More handled by View JS.');

    await app.view.testId("native-menu-toggle", { page: 'video' }).click();
    await eventually(
      () => moreButton.query(),
      (button) => button.exists && button.visible,
      { timeoutMs: 5_000, describe: 'native menu remounted for keyboard activation' },
    );
    await desktop.ax.focus({ window: host.id, match: 'name:More native menu actions' });
    await eventually(
      () => app.view.eval({ page: 'video', timeout: 5_000 }, ({ document }) => document.activeElement?.id ?? null),
      (value) => value === 'video-native-menu-more',
      { timeoutMs: 5_000, describe: 'Windows UIA focus reached the native menu More element' },
    );
    await app.view.eval({ page: 'video' }, ({ document, window }) => {
      const page = window as unknown as FrameProbeWindow;
      return (document.activeElement as ProbeElement | null)
        ?.dispatchEvent(new page.KeyboardEvent('keydown', { key: 'Enter', bubbles: true, cancelable: true })) ?? null;
    });
    expect(await waitForElementText(
      t,
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
  await app.view.testId('video-page', { page: 'video' }).waitFor({ state: 'visible', timeout: 30_000 });
  await app.view.css('#lx-video-shape-fixture', { page: 'video' }).first().waitFor({ state: 'visible', timeout: 30_000 });
  // The shape fixture loads no media, and only Apple emits a pause event
  // without a playing transition; just exercise the pause command itself.
  await app.view.testId("video-pause", { page: 'video' }).click();
  await attachWindow(t, 'native-video-active.png');

  const hiddenAt = Date.now();
  await app.nav.back();
  await waitForCurrentPageVisible(app, 'home', '[data-testid="home-page"]', 5_000);

  const name = `Native overlay ${namespace}`;
  await app.view.testId("home-name", { page: 'home' }).fill(name);
  await app.view.testId("home-greet", { page: 'home' }).click();
  expect(await waitForElementText(
    t,
    'home',
    '[data-testid="home-greeting"]',
    (text) => text.includes(name),
    5_000,
  )).toContain(name);
  expect(Date.now() - hiddenAt).toBeLessThan(5_000);
  await attachWindow(t, 'native-video-hidden.png');
});
