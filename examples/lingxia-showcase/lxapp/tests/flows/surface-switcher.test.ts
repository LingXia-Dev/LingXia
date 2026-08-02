import { expect, test } from '@rongjs/test';
import type {
  AutomationShellPin,
  DesktopWindowInfo,
  LxAppDriver,
  SurfaceLayoutAsideSlot,
  SurfaceLayoutSnapshot,
} from 'lingxia-types';
import { runtimePlatform } from '../helpers/platform.js';

interface VisibilityEvent {
  id: string;
  kind: string;
  source: string;
}

interface CloseEvent {
  id: string;
  kind: string;
  reason: string;
}

const targetPlatform = (test.args as Record<string, string>).platform?.toLocaleLowerCase();
const desktopTest = targetPlatform && !['macos', 'windows'].includes(targetPlatform)
  ? test.skip
  : test;
const windowsHostTest = targetPlatform === 'windows' ? test : test.skip;

async function desktopApp(): Promise<LxAppDriver> {
  const app = lx.automation().lxapp();
  const actual = await runtimePlatform(app);
  if (!['macos', 'windows'].includes(actual)) {
    throw new Error(
      `surface-switcher tests require macOS or Windows; got ${actual || 'unknown'}. `
      + 'Pass --arg platform=<target> so non-desktop matrix jobs skip explicitly.',
    );
  }
  if (targetPlatform && targetPlatform !== actual) {
    throw new Error(`requested ${targetPlatform}, but the running showcase reports ${actual}`);
  }
  return app;
}

function switcherIds(layout: SurfaceLayoutSnapshot): string[] {
  return layout.mainSwitcher.items.map((item) => item.surfaceId);
}

function nativeSlot(layout: SurfaceLayoutSnapshot): SurfaceLayoutAsideSlot | undefined {
  return layout.asideSlots.find((slot) => slot.kind === 'native');
}

function containsSurface(layout: SurfaceLayoutSnapshot, id: string): boolean {
  return layout.mains.includes(id)
    || layout.asideSlots.some((slot) => slot.children.includes(id))
    || layout.floats.some((surface) => surface.id === id);
}

function topology(layout: SurfaceLayoutSnapshot): unknown {
  return {
    sizeClass: layout.sizeClass,
    switcherForm: layout.switcherForm,
    splitForm: layout.splitForm,
    mains: layout.mains,
    activeMainId: layout.activeMainId,
    rootSurfaceId: layout.mainSwitcher.rootSurfaceId,
    activeSurfaceId: layout.mainSwitcher.activeSurfaceId,
    switcher: layout.mainSwitcher.items.map((item) => ({
      surfaceId: item.surfaceId,
      active: item.active,
      root: item.root,
      closable: item.closable,
    })),
    asides: layout.asides,
    asideSlots: layout.asideSlots,
    floats: layout.floats,
    tree: layout.tree,
  };
}

function windowContains(outer: DesktopWindowInfo, inner: DesktopWindowInfo): boolean {
  const tolerance = 8;
  return inner.bounds.x >= outer.bounds.x - tolerance
    && inner.bounds.y >= outer.bounds.y - tolerance
    && inner.bounds.x + inner.bounds.w <= outer.bounds.x + outer.bounds.w + tolerance
    && inner.bounds.y + inner.bounds.h <= outer.bounds.y + outer.bounds.h + tolerance;
}

function windowsHost(windows: DesktopWindowInfo[]): DesktopWindowInfo | undefined {
  return windows
    .filter((window) => (
      window.visible
      && window.title === 'LingXia'
      && window.process.toLocaleLowerCase() !== 'msedgewebview2'
    ))
    .sort((left, right) => (
      right.bounds.w * right.bounds.h - left.bounds.w * left.bounds.h
    ))[0];
}

function visibleHostWebViews(
  host: DesktopWindowInfo,
  windows: DesktopWindowInfo[],
): DesktopWindowInfo[] {
  return windows.filter((window) => (
    window.visible
    && window.process.toLocaleLowerCase() === 'msedgewebview2'
    && windowContains(host, window)
    && window.bounds.w > 0
    && window.bounds.h > 0
  ));
}

function visibleWorkspaceHosts(
  host: DesktopWindowInfo,
  windows: DesktopWindowInfo[],
): DesktopWindowInfo[] {
  return windows.filter((window) => (
    window.visible
    && window.pid === host.pid
    && window.title === 'LingXia'
    && window.process.toLocaleLowerCase() !== 'msedgewebview2'
  ));
}

function expectSingleWorkspaceHost(
  host: DesktopWindowInfo,
  windows: DesktopWindowInfo[],
): void {
  expect(visibleWorkspaceHosts(host, windows).map((window) => window.id)).toEqual([host.id]);
}

async function expectExactMainPresentation(
  host: DesktopWindowInfo,
  baseline: DesktopWindowInfo,
  active: DesktopWindowInfo,
  readWindows: () => Promise<DesktopWindowInfo[]>,
): Promise<void> {
  // A page-owned native navigation bar may shorten the inner WebView at the
  // top. The host presentation still has to share the root's left, right, and
  // bottom edges, with no outgoing WebView or duplicate workspace left visible.
  expect(active.bounds.x).toBe(baseline.bounds.x);
  expect(active.bounds.w).toBe(baseline.bounds.w);
  expect(active.bounds.y >= baseline.bounds.y).toBeTruthy();
  expect(active.bounds.y + active.bounds.h).toBe(
    baseline.bounds.y + baseline.bounds.h,
  );
  // WebView2 commits controller visibility asynchronously even though the
  // host call is synchronous. Require physical convergence instead of
  // sampling that commit boundary once; a controller that remains exposed
  // still fails this production gate at the deadline.
  const windows = await waitForValue(async () => {
    const candidate = await readWindows();
    const visible = visibleHostWebViews(host, candidate);
    return visible.length === 1 && visible[0].id === active.id
      ? candidate
      : undefined;
  }, 'outgoing main WebView hidden');
  expectSingleWorkspaceHost(host, windows);
}

function expectOverlayCoversMain(
  host: DesktopWindowInfo,
  baseline: DesktopWindowInfo,
  overlay: DesktopWindowInfo,
  windows: DesktopWindowInfo[],
): void {
  const hostWebViews = visibleHostWebViews(host, windows);
  const visibleOverlay = hostWebViews.find((window) => window.id === overlay.id);
  expect(Boolean(visibleOverlay)).toBeTruthy();
  expect(visibleOverlay!.bounds.x).toBe(baseline.bounds.x);
  expect(visibleOverlay!.bounds.w).toBe(baseline.bounds.w);
  expect(visibleOverlay!.bounds.y >= baseline.bounds.y).toBeTruthy();
  expect(visibleOverlay!.bounds.y + visibleOverlay!.bounds.h).toBe(
    baseline.bounds.y + baseline.bounds.h,
  );
  const visibleMain = hostWebViews.find((window) => (
    window.id === baseline.id
  ));
  expect(Boolean(visibleMain)).toBeTruthy();
  expectSingleWorkspaceHost(host, windows);
}

function samePin(left: AutomationShellPin, right: AutomationShellPin): boolean {
  return left.kind === right.kind && left.key === right.key;
}

function pinnedShortcutPoint(
  host: DesktopWindowInfo,
  index: number,
): [number, number] {
  const tile = 36;
  const gap = 5;
  const columns = 4;
  const sidebarWidth = 184;
  const gridWidth = columns * tile + (columns - 1) * gap;
  const gridLeft = Math.floor((sidebarWidth - gridWidth) / 2);
  const row = Math.floor(index / columns);
  const column = index % columns;
  return [
    host.bounds.x + gridLeft + column * (tile + gap) + tile / 2,
    host.bounds.y + 32 + row * (tile + gap) + tile / 2,
  ];
}

function showcaseHomePagePoint(
  host: DesktopWindowInfo,
  pinCount: number,
): [number, number] {
  const pinRows = Math.ceil(pinCount / 4);
  const pinnedGridHeight = pinRows * (36 + 5);
  const topBarHeight = 32;
  const groupHeight = 36;
  const parentChildGap = 1;
  const childHeight = 28;
  return [
    host.bounds.x + 84,
    host.bounds.y
      + topBarHeight
      + pinnedGridHeight
      + groupHeight
      + parentChildGap
      + childHeight / 2,
  ];
}

async function waitForValue<T>(
  read: () => Promise<T | undefined>,
  label: string,
  timeoutMs = 10_000,
): Promise<T> {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    const value = await read();
    if (value !== undefined) return value;
    await new Promise((resolve) => setTimeout(resolve, 50));
  }
  throw new Error(`${label} was not observed within ${timeoutMs}ms`);
}

async function waitForDesktopWindow(
  read: () => Promise<DesktopWindowInfo[]>,
  select: (windows: DesktopWindowInfo[]) => DesktopWindowInfo | undefined,
  label: string,
  timeoutMs = 10_000,
): Promise<DesktopWindowInfo> {
  let observed: DesktopWindowInfo[] = [];
  try {
    return await waitForValue(async () => {
      observed = await read();
      return select(observed);
    }, label, timeoutMs);
  } catch (error) {
    const visible = observed.filter((window) => window.visible).map((window) => ({
      id: window.id,
      title: window.title,
      process: window.process,
      bounds: window.bounds,
      z: window.z,
    }));
    throw new Error(`${String(error)}; visible windows: ${JSON.stringify(visible)}`);
  }
}

async function closeChatSurface(app: LxAppDriver): Promise<void> {
  await app.eval({
    timeoutMs: 20_000,
    script: `
      const layout = await lx.automation().lxapp().surfaceLayout();
      let chat;
      if (layout.mains.includes('lingxia-chat')) {
        chat = await lx.openSurface({ surface: 'lingxia-chat', as: 'main' });
      } else if (layout.asideSlots.some((slot) => slot.children.includes('lingxia-chat'))) {
        chat = await lx.openSurface({ surface: 'lingxia-chat' });
      }
      if (chat?.alive) await chat.close();
    `,
  });
}

desktopTest('projects the declared terminal aside and restores its baseline state', async () => {
  const app = await desktopApp();
  const before = await app.surfaceLayout();
  const result = await app.eval({
    timeoutMs: 30_000,
    script: `
      const driver = lx.automation().lxapp();
      const snapshot = () => driver.surfaceLayout();
      const settle = () => new Promise((resolve) => setTimeout(resolve, 100));
      const before = await snapshot();
      const existed = before.asideSlots.some((slot) => slot.children.includes('terminal'));
      const wasVisible = before.asides.some((surface) => surface.id === 'terminal');
      const terminal = await lx.openSurface({ surface: 'terminal' });
      const visibility = { hide: [], show: [] };
      const off = [
        terminal.onHide((event) => visibility.hide.push(event)),
        terminal.onShow((event) => visibility.show.push(event)),
      ];
      let output;
      try {
        const opened = await snapshot();
        await terminal.hide();
        await terminal.hide();
        const hidden = await snapshot();
        await terminal.show();
        await terminal.show();
        await settle();
        const shown = await snapshot();
        output = {
          id: terminal.id,
          role: terminal.role,
          presentation: terminal.presentation,
          opened,
          hidden,
          shown,
          visibility: {
            hide: visibility.hide.map((event) => ({ ...event })),
            show: visibility.show.map((event) => ({ ...event })),
          },
        };
      } finally {
        for (const unsubscribe of off) unsubscribe();
        if (existed) {
          if (wasVisible) await terminal.show();
          else await terminal.hide();
        } else if (terminal.alive) {
          await terminal.close();
        }
      }
      output.afterCleanup = await snapshot();
      return output;
    `,
  }) as {
    id: string;
    role: string;
    presentation: string;
    opened: SurfaceLayoutSnapshot;
    hidden: SurfaceLayoutSnapshot;
    shown: SurfaceLayoutSnapshot;
    visibility: { hide: VisibilityEvent[]; show: VisibilityEvent[] };
    afterCleanup: SurfaceLayoutSnapshot;
  };

  expect(result.id).toBe('terminal');
  expect(result.role).toBe('aside');
  expect(['dock', 'overlay']).toContain(result.presentation);
  expect(result.opened.asides.some((surface) => (
    surface.id === result.id && surface.edge === 'bottom'
  ))).toBeTruthy();
  expect(nativeSlot(result.opened)?.children.includes(result.id)).toBeTruthy();
  expect(nativeSlot(result.opened)?.activeChild).toBe(result.id);
  expect(result.hidden.asides.some((surface) => surface.id === result.id)).toBeFalsy();
  expect(nativeSlot(result.hidden)?.children.includes(result.id)).toBeTruthy();
  expect(nativeSlot(result.hidden)?.activeChild === result.id).toBeFalsy();
  expect(result.shown.asides.some((surface) => surface.id === result.id)).toBeTruthy();
  expect(nativeSlot(result.shown)?.activeChild).toBe(result.id);
  expect(result.visibility.hide).toEqual([
    { id: result.id, kind: 'overlay', source: 'opener' },
  ]);
  expect(result.visibility.show).toEqual([
    { id: result.id, kind: 'overlay', source: 'opener' },
  ]);
  expect(topology(result.afterCleanup)).toEqual(topology(before));
});

windowsHostTest('docks the footer Chat WebView physically beside the main after resize', async () => {
  const app = await desktopApp();
  const automation = lx.automation();
  const desktop = automation.desktop;
  const doctor = await desktop.doctor();
  expect(doctor.capabilities.windows).toBeTruthy();
  expect(doctor.capabilities.pointer).toBeTruthy();
  expect(doctor.capabilities.window_management).toBeTruthy();

  let host = windowsHost(await desktop.windows());
  if (!host) throw new Error('visible LingXia host window was not found');
  const originalBounds = { ...host.bounds };
  const overlayWidth = Math.round(720 * host.scale);
  const dockedWidth = Math.round(1_200 * host.scale);
  let chatOpened = false;
  try {
    // Window bounds are physical pixels while surface breakpoints are DIPs.
    // Target explicit logical widths so this covers the same medium/expanded
    // handoff at every runner DPI.
    host = await desktop.window.resize({
      window: host.id,
      width: overlayWidth,
      height: 768,
    });
    await desktop.window.focus({ window: host.id });

    const baseline = await app.surfaceLayout();
    expect(containsSurface(baseline, 'lingxia-chat')).toBeFalsy();
    const baselineWindows = await desktop.windows();
    expectSingleWorkspaceHost(host, baselineWindows);
    const baselineWebViews = visibleHostWebViews(host, baselineWindows);
    expect(baselineWebViews.length).toBe(1);
    const baselineMain = baselineWebViews[0];
    const baselineWebViewIds = new Set(baselineWebViews.map((window) => window.id));

    // Exercise the real native footer action. Its first cell is anchored to
    // the lower-left of the fixed desktop sidebar; use host-relative geometry
    // so the assertion is independent of monitor origin and DPI.
    await desktop.pointer.click({
      at: [host.bounds.x + 50, host.bounds.y + host.bounds.h - 54],
    });
    chatOpened = true;

    const overlayLayout = await waitForValue(async () => {
      const layout = await app.surfaceLayout();
      const slot = layout.asideSlots.find((candidate) => (
        candidate.activeChild === 'lingxia-chat'
      ));
      return layout.sizeClass === 'medium' && slot?.visible && slot.overlay
        ? layout
        : undefined;
    }, 'footer Chat aside');
    expect(overlayLayout.activeMainId).toBe('lingxia-showcase');
    expect(overlayLayout.mains.includes('lingxia-chat')).toBeFalsy();
    expect(switcherIds(overlayLayout).includes('lingxia-chat')).toBeFalsy();
    expect(overlayLayout.asideSlots.find((slot) => (
      slot.children.includes('lingxia-chat')
    ))?.overlay).toBeTruthy();

    const overlayWindow = await waitForDesktopWindow(
      () => desktop.windows(),
      (windows) => visibleHostWebViews(host!, windows).find((window) => (
        !baselineWebViewIds.has(window.id)
        && window.bounds.w > host!.bounds.w * 0.55
        && window.bounds.h > host!.bounds.h * 0.55
      )),
      'Chat WebView in the physical medium overlay',
    );
    expectOverlayCoversMain(
      host,
      baselineMain,
      overlayWindow,
      await desktop.windows(),
    );

    // Prove the composed overlay is actually the input target, not merely a
    // visible controller behind the main. Page automation only discovers the
    // textarea; the click and typing travel through the real desktop stack.
    const chatApp = automation.lxapp('lingxia-chat');
    const chatInput = await waitForValue(async () => {
      try {
        const candidate = await chatApp.page.query({
          page: 'chat',
          css: 'textarea[placeholder="Message..."]',
        });
        return candidate.exists && candidate.visible && candidate.editable
          ? candidate
          : undefined;
      } catch (error) {
        if (String(error).includes('page WebView is not ready')) return undefined;
        throw error;
      }
    }, 'Chat overlay input');
    const inputPoint: [number, number] = [
      overlayWindow.bounds.x + Math.round(chatInput.rect.center_x * overlayWindow.scale),
      overlayWindow.bounds.y + Math.round(chatInput.rect.center_y * overlayWindow.scale),
    ];
    const inputMarker = 'physical-overlay-front';
    await desktop.pointer.click({ at: inputPoint });
    await desktop.key.type({ text: inputMarker });
    await waitForValue(async () => {
      const candidate = await chatApp.page.query({
        page: 'chat',
        css: 'textarea[placeholder="Message..."]',
      });
      return candidate.exists && candidate.value === inputMarker ? true : undefined;
    }, 'desktop input delivered to Chat overlay');
    await chatApp.page.fill({
      page: 'chat',
      css: 'textarea[placeholder="Message..."]',
      text: '',
    });

    host = await desktop.window.resize({
      window: host.id,
      width: dockedWidth,
      height: 900,
    });
    const dockedLayout = await waitForValue(async () => {
      const layout = await app.surfaceLayout();
      const slot = layout.asideSlots.find((candidate) => (
        candidate.children.includes('lingxia-chat')
      ));
      return layout.sizeClass === 'expanded' && slot?.visible && !slot.overlay
        ? layout
        : undefined;
    }, 'expanded docked Chat layout');
    expect(dockedLayout.activeMainId).toBe('lingxia-showcase');

    const chatWindow = await waitForDesktopWindow(
      () => desktop.windows(),
      (windows) => visibleHostWebViews(host!, windows).find((window) => (
        window.id === overlayWindow.id
        && window.bounds.x > host!.bounds.x + host!.bounds.w * 0.55
        && window.bounds.w < host!.bounds.w * 0.45
      )),
      'Chat WebView in the physical right aside',
    );

    const dockedWindows = await desktop.windows();
    const visibleMain = visibleHostWebViews(host, dockedWindows).find((window) => (
      baselineWebViewIds.has(window.id)
      && window.bounds.x < chatWindow.bounds.x
    ));
    expect(Boolean(visibleMain)).toBeTruthy();
    expectSingleWorkspaceHost(host, dockedWindows);

    const capture = await automation.lxapps.screenshot();
    expect(capture.width >= host.bounds.w - 2).toBeTruthy();
    expect(capture.height >= host.bounds.h - 2).toBeTruthy();

    await closeChatSurface(app);
    chatOpened = false;
    await waitForValue(async () => (
      containsSurface(await app.surfaceLayout(), 'lingxia-chat') ? undefined : true
    ), 'closed Chat after adaptive handoff');
    const restoredMain = await waitForDesktopWindow(
      () => desktop.windows(),
      (windows) => {
        const visible = visibleHostWebViews(host!, windows);
        return visible.length === 1 && visible[0].id === baselineMain.id
          ? visible[0]
          : undefined;
      },
      'restored main after closing adaptive Chat',
    );
    await expectExactMainPresentation(
      host,
      baselineMain,
      restoredMain,
      () => desktop.windows(),
    );
  } finally {
    if (chatOpened) {
      await app.eval({
        timeoutMs: 20_000,
        script: `
          const chat = await lx.openSurface({ surface: 'lingxia-chat' });
          if (chat.alive) await chat.close();
        `,
      });
    }
    await desktop.window.resize({
      window: host.id,
      width: originalBounds.w,
      height: originalBounds.h,
    });
  }
});

windowsHostTest('opens a pinned lxapp as an exact main workspace and keeps its menu live', async () => {
  const app = await desktopApp();
  const automation = lx.automation();
  const desktop = automation.desktop;
  const shell = automation.shell;
  const targetPin = { kind: 'lxapp', key: 'lingxia-chat' } as const;
  const initialPins = await shell.pins();
  const initiallyPinned = initialPins.some((pin) => samePin(pin, targetPin));
  if (!initiallyPinned && initialPins.length >= 8) {
    throw new Error('pinned lxapp gate requires one free Pin slot');
  }

  let host = windowsHost(await desktop.windows());
  if (!host) throw new Error('visible LingXia host window was not found');
  const originalBounds = { ...host.bounds };
  const originalPage = (await app.info()).current_page;
  const originalPageName = (await app.pages()).find((page) => (
    originalPage?.startsWith(page.path)
  ))?.name;
  const dockedWidth = Math.round(1_200 * host.scale);
  try {
    await closeChatSurface(app);
    await waitForValue(async () => (
      containsSurface(await app.surfaceLayout(), 'lingxia-chat') ? undefined : true
    ), 'closed Chat baseline');
    await app.nav.switchTab({ page: 'todo' });

    const pins = await shell.setPin({ ...targetPin, pinned: true });
    const pinIndex = pins.findIndex((pin) => samePin(pin, targetPin));
    expect(pinIndex >= 0).toBeTruthy();

    host = await desktop.window.resize({
      window: host.id,
      width: dockedWidth,
      height: 900,
    });
    await desktop.window.focus({ window: host.id });
    await waitForValue(async () => {
      const candidate = await app.surfaceLayout();
      return candidate.sizeClass === 'expanded'
        && candidate.activeMainId === 'lingxia-showcase'
        ? candidate
        : undefined;
    }, 'expanded main baseline');

    const baselineMain = await waitForDesktopWindow(
      () => desktop.windows(),
      (windows) => {
        const visible = visibleHostWebViews(host!, windows);
        return visible.length === 1
          && visible[0].bounds.w > host!.bounds.w * 0.6
          && visible[0].bounds.h > host!.bounds.h * 0.6
          ? visible[0]
          : undefined;
      },
      'Showcase physical main bounds',
    );
    expectSingleWorkspaceHost(host, await desktop.windows());

    // Start from the declared entry so this gate covers the difficult case:
    // a Pin must promote the one live aside instance into a main workspace.
    await app.eval({
      timeoutMs: 20_000,
      script: `await lx.openSurface({ surface: 'lingxia-chat' });`,
    });
    const declaredAside = await waitForValue(async () => {
      const candidate = await app.surfaceLayout();
      const slot = candidate.asideSlots.find((item) => (
        item.activeChild === 'lingxia-chat' && item.visible && !item.overlay
      ));
      return candidate.sizeClass === 'expanded' && slot ? candidate : undefined;
    }, 'declared Chat right aside');
    expect(declaredAside.activeMainId).toBe('lingxia-showcase');
    expect(switcherIds(declaredAside).includes('lingxia-chat')).toBeFalsy();

    const declaredAsideWindow = await waitForDesktopWindow(
      () => desktop.windows(),
      (windows) => visibleHostWebViews(host!, windows).find((window) => (
        window.id !== baselineMain.id
        && window.bounds.x > host!.bounds.x + host!.bounds.w * 0.55
        && window.bounds.w < host!.bounds.w * 0.45
      )),
      'declared Chat physical right aside',
    );
    expect(declaredAsideWindow.id === baselineMain.id).toBeFalsy();
    expectSingleWorkspaceHost(host, await desktop.windows());

    const pinPoint = pinnedShortcutPoint(host, pinIndex);
    await desktop.pointer.click({ at: pinPoint });
    const promotedLayout = await waitForValue(async () => {
      const candidate = await app.surfaceLayout();
      return candidate.activeMainId === 'lingxia-chat'
        && candidate.mains.includes('lingxia-chat')
        && switcherIds(candidate).includes('lingxia-chat')
        && !candidate.asideSlots.some((slot) => slot.children.includes('lingxia-chat'))
        ? candidate
        : undefined;
    }, 'pinned Chat promoted main workspace');
    expect(promotedLayout.mainSwitcher.activeSurfaceId).toBe('lingxia-chat');

    const promotedMain = await waitForDesktopWindow(
      () => desktop.windows(),
      (windows) => visibleHostWebViews(host!, windows).find((window) => (
        window.id !== baselineMain.id
        && window.bounds.w > host!.bounds.w * 0.6
        && window.bounds.h > host!.bounds.h * 0.6
      )),
      'promoted Chat physical main',
    );
    await expectExactMainPresentation(
      host,
      baselineMain,
      promotedMain,
      () => desktop.windows(),
    );

    // Close and repeat from cold state: the same physical Pin must still
    // create a switchable main and occupy exactly the root content rectangle.
    await closeChatSurface(app);
    await waitForValue(async () => {
      const candidate = await app.surfaceLayout();
      return !containsSurface(candidate, 'lingxia-chat')
        && candidate.activeMainId === 'lingxia-showcase'
        ? candidate
        : undefined;
    }, 'closed promoted Chat workspace');
    const restoredAfterClose = await waitForDesktopWindow(
      () => desktop.windows(),
      (windows) => {
        const visible = visibleHostWebViews(host!, windows);
        return visible.length === 1 && visible[0].id === baselineMain.id
          ? visible[0]
          : undefined;
      },
      'root main restored after closing promoted Chat',
    );
    await expectExactMainPresentation(
      host,
      baselineMain,
      restoredAfterClose,
      () => desktop.windows(),
    );
    await desktop.pointer.click({ at: pinPoint });
    const coldLayout = await waitForValue(async () => {
      const candidate = await app.surfaceLayout();
      return candidate.activeMainId === 'lingxia-chat'
        && candidate.mains.includes('lingxia-chat')
        && switcherIds(candidate).includes('lingxia-chat')
        && !candidate.asideSlots.some((slot) => slot.children.includes('lingxia-chat'))
        ? candidate
        : undefined;
    }, 'cold pinned Chat main workspace');
    expect(coldLayout.mainSwitcher.activeSurfaceId).toBe('lingxia-chat');

    const coldMain = await waitForDesktopWindow(
      () => desktop.windows(),
      (windows) => visibleHostWebViews(host!, windows).find((window) => (
        window.id !== baselineMain.id
        && window.bounds.w > host!.bounds.w * 0.6
        && window.bounds.h > host!.bounds.h * 0.6
      )),
      'cold pinned Chat physical main',
    );
    await expectExactMainPresentation(
      host,
      baselineMain,
      coldMain,
      () => desktop.windows(),
    );

    const windowsBeforeMenu = await desktop.windows();
    const existingWindowIds = new Set(windowsBeforeMenu.map((window) => window.id));
    await desktop.pointer.click({ at: pinPoint, button: 'right' });
    const menu = await waitForValue(async () => (
      (await desktop.windows()).find((window) => (
        window.visible
        && !existingWindowIds.has(window.id)
        && window.process.toLocaleLowerCase() === host!.process.toLocaleLowerCase()
        && window.title === ''
        && Math.abs(window.bounds.x - pinPoint[0]) <= 8
        && Math.abs(window.bounds.y - pinPoint[1]) <= 8
        && window.bounds.w > 0
        && window.bounds.w < host!.bounds.w
        && window.bounds.h > 0
        && window.bounds.h < host!.bounds.h
      ))
    ), 'native pinned Chat context menu');
    if (initiallyPinned) {
      await desktop.key.press({ key: 'Escape' });
    } else {
      // The informational header is disabled, so ArrowDown selects Unpin
      // regardless of display language. Enter proves the real native menu
      // opened and dispatched its command rather than merely hit-testing.
      await desktop.key.press({ key: 'ArrowDown' });
      await desktop.key.press({ key: 'Enter' });
      await waitForValue(async () => (
        (await shell.pins()).some((pin) => samePin(pin, targetPin)) ? undefined : true
      ), 'pinned Chat context-menu Unpin');
    }
    await waitForValue(async () => (
      (await desktop.windows()).some((window) => window.id === menu.id) ? undefined : true
    ), 'dismissed pinned Chat context menu');

    const pinsAfterMenu = await shell.pins();
    await desktop.pointer.click({
      at: showcaseHomePagePoint(host, pinsAfterMenu.length),
    });
    await waitForValue(async () => (
      (await app.info()).current_page?.startsWith('pages/home/index') ? true : undefined
    ), 'responsive sidebar after pinned Chat context menu');
    const afterSidebarClick = await app.surfaceLayout();
    expect(afterSidebarClick.activeMainId).toBe('lingxia-showcase');
    expect(afterSidebarClick.mains.includes('lingxia-chat')).toBeTruthy();
    expect(switcherIds(afterSidebarClick).includes('lingxia-chat')).toBeTruthy();
    expect(afterSidebarClick.asideSlots.some((slot) => (
      slot.children.includes('lingxia-chat')
    ))).toBeFalsy();
    const rootAfterSidebarSwitch = await waitForDesktopWindow(
      () => desktop.windows(),
      (windows) => {
        const visible = visibleHostWebViews(host!, windows);
        return visible.length === 1 && visible[0].id === baselineMain.id
          ? visible[0]
          : undefined;
      },
      'root main after sidebar switch from pinned Chat',
    );
    await expectExactMainPresentation(
      host,
      baselineMain,
      rootAfterSidebarSwitch,
      () => desktop.windows(),
    );
  } finally {
    await desktop.key.press({ key: 'Escape' }).catch(() => undefined);
    await closeChatSurface(app).catch(() => undefined);
    await shell.setPin({ ...targetPin, pinned: initiallyPinned });
    await desktop.window.resize({
      window: host.id,
      width: originalBounds.w,
      height: originalBounds.h,
    });
    if (originalPageName) {
      await app.nav.relaunch({ page: originalPageName });
    }
  }
});

desktopTest('rejects stable-root mutations without changing the host model', async () => {
  const app = await desktopApp();
  const result = await app.eval({
    timeoutMs: 20_000,
    script: `
      const driver = lx.automation().lxapp();
      const snapshot = () => driver.surfaceLayout();
      const initial = await snapshot();
      const rootId = initial.mainSwitcher.rootSurfaceId;
      if (!rootId) throw new Error('surface graph has no stable root');
      const root = await lx.openSurface({ surface: rootId, as: 'main' });
      const beforeRejections = await snapshot();
      let closeError = '';
      try { await root.close(); } catch (error) { closeError = String(error); }
      let roleError = '';
      try {
        await lx.openSurface({ surface: rootId, as: 'aside', edge: 'right' });
      } catch (error) {
        roleError = String(error);
      }
      return {
        rootId,
        closeError,
        roleError,
        role: root.role,
        visible: root.visible,
        alive: root.alive,
        beforeRejections,
        afterRejections: await snapshot(),
      };
    `,
  }) as {
    rootId: string;
    closeError: string;
    roleError: string;
    role: string;
    visible: boolean;
    alive: boolean;
    beforeRejections: SurfaceLayoutSnapshot;
    afterRejections: SurfaceLayoutSnapshot;
  };

  expect(result.closeError).toContain('stable root main surface cannot be closed');
  expect(result.roleError).toContain('stable root main surface cannot change role');
  expect(result.role).toBe('main');
  expect(result.visible).toBeTruthy();
  expect(result.alive).toBeTruthy();
  expect(result.afterRejections.mainSwitcher.revision)
    .toBe(result.beforeRejections.mainSwitcher.revision);
  expect(topology(result.afterRejections)).toEqual(topology(result.beforeRejections));
  const rootItem = result.afterRejections.mainSwitcher.items.find((item) => (
    item.surfaceId === result.rootId
  ));
  expect(rootItem?.root).toBeTruthy();
  expect(rootItem?.closable).toBeFalsy();
});

desktopTest('migrates one keyed workspace across aside edges and main exactly once', async () => {
  const app = await desktopApp();
  const before = await app.surfaceLayout();
  const key = `automation-migrate-${Date.now()}-${Math.random().toString(36).slice(2)}`;
  const result = await app.eval({
    timeoutMs: 45_000,
    script: `
      const driver = lx.automation().lxapp();
      const snapshot = () => driver.surfaceLayout();
      const settle = () => new Promise((resolve) => setTimeout(resolve, 100));
      const key = ${JSON.stringify(key)};
      const visibility = { hide: [], show: [] };
      const closed = [];
      let surface;
      const off = [];
      let output;
      try {
        surface = await lx.openSurface({
          surface: 'terminal', key, as: 'aside', edge: 'right',
        });
        off.push(
          surface.onHide((event) => visibility.hide.push(event)),
          surface.onShow((event) => visibility.show.push(event)),
          surface.onClose((event) => closed.push(event)),
        );
        const aside = await snapshot();
        const main = await lx.openSurface({ surface: 'terminal', key, as: 'main' });
        const mainLayout = await snapshot();
        const roleAfterMain = main.role;
        let hideError = '';
        try { await main.hide(); } catch (error) { hideError = String(error); }
        const afterRejectedHide = await snapshot();
        const docked = await lx.openSurface({
          surface: 'terminal', key, as: 'aside', edge: 'bottom',
        });
        const dockedLayout = await snapshot();
        await docked.hide();
        await docked.hide();
        const hiddenLayout = await snapshot();
        await docked.show();
        await docked.show();
        const shownLayout = await snapshot();
        await docked.close();
        await settle();
        const afterClose = await snapshot();
        const revisionAfterClose = afterClose.mainSwitcher.revision;
        await docked.close();
        await settle();
        const afterRepeatedClose = await snapshot();
        output = {
          id: surface.id,
          sameMainId: main.id === surface.id,
          sameMainHandle: main === surface,
          sameAsideId: docked.id === surface.id,
          sameAsideHandle: docked === surface,
          roleAfterMain,
          roleAfterDock: docked.role,
          presentationAfterDock: docked.presentation,
          hideError,
          aliveAfterClose: docked.alive,
          visibleAfterClose: docked.visible,
          aside,
          mainLayout,
          afterRejectedHide,
          dockedLayout,
          hiddenLayout,
          shownLayout,
          afterClose,
          afterRepeatedClose,
          revisionAfterClose,
          visibility: {
            hide: visibility.hide.map((event) => ({ ...event })),
            show: visibility.show.map((event) => ({ ...event })),
          },
          closed: closed.map((event) => ({ ...event })),
        };
      } finally {
        for (const unsubscribe of off.splice(0)) unsubscribe();
        if (surface?.alive) await surface.close();
      }
      output.afterCleanup = await snapshot();
      return output;
    `,
  }) as {
    id: string;
    sameMainId: boolean;
    sameMainHandle: boolean;
    sameAsideId: boolean;
    sameAsideHandle: boolean;
    roleAfterMain: string;
    roleAfterDock: string;
    presentationAfterDock: string;
    hideError: string;
    aliveAfterClose: boolean;
    visibleAfterClose: boolean;
    aside: SurfaceLayoutSnapshot;
    mainLayout: SurfaceLayoutSnapshot;
    afterRejectedHide: SurfaceLayoutSnapshot;
    dockedLayout: SurfaceLayoutSnapshot;
    hiddenLayout: SurfaceLayoutSnapshot;
    shownLayout: SurfaceLayoutSnapshot;
    afterClose: SurfaceLayoutSnapshot;
    afterRepeatedClose: SurfaceLayoutSnapshot;
    afterCleanup: SurfaceLayoutSnapshot;
    revisionAfterClose: number;
    visibility: { hide: VisibilityEvent[]; show: VisibilityEvent[] };
    closed: CloseEvent[];
  };

  expect(result.sameMainId).toBeTruthy();
  expect(result.sameMainHandle).toBeTruthy();
  expect(result.sameAsideId).toBeTruthy();
  expect(result.sameAsideHandle).toBeTruthy();
  expect(result.aside.mains.includes(result.id)).toBeFalsy();
  expect(result.aside.asides.some((surface) => (
    surface.id === result.id && surface.edge === 'right'
  ))).toBeTruthy();
  expect(nativeSlot(result.aside)?.edge).toBe('right');
  expect(nativeSlot(result.aside)?.activeChild).toBe(result.id);
  expect(result.mainLayout.mains.includes(result.id)).toBeTruthy();
  expect(result.mainLayout.asides.some((surface) => surface.id === result.id)).toBeFalsy();
  expect(result.mainLayout.asideSlots.some((slot) => slot.children.includes(result.id))).toBeFalsy();
  expect(result.mainLayout.activeMainId).toBe(result.id);
  expect(result.mainLayout.mainSwitcher.activeSurfaceId).toBe(result.id);
  const mainItem = result.mainLayout.mainSwitcher.items.find((item) => (
    item.surfaceId === result.id
  ));
  expect(mainItem?.active).toBeTruthy();
  expect(mainItem?.root).toBeFalsy();
  expect(mainItem?.closable).toBeTruthy();
  expect(mainItem?.content).toEqual({ kind: 'native', capability: 'terminal' });
  expect(result.roleAfterMain).toBe('main');
  expect(result.hideError).toContain('main surface cannot be hidden');
  expect(result.afterRejectedHide.mainSwitcher.revision)
    .toBe(result.mainLayout.mainSwitcher.revision);
  expect(topology(result.afterRejectedHide)).toEqual(topology(result.mainLayout));
  expect(result.dockedLayout.mains.includes(result.id)).toBeFalsy();
  expect(result.dockedLayout.asides.some((surface) => (
    surface.id === result.id && surface.edge === 'bottom'
  ))).toBeTruthy();
  expect(nativeSlot(result.dockedLayout)?.edge).toBe('bottom');
  expect(nativeSlot(result.dockedLayout)?.activeChild).toBe(result.id);
  expect(result.roleAfterDock).toBe('aside');
  expect(['dock', 'overlay']).toContain(result.presentationAfterDock);
  expect(result.hiddenLayout.asides.some((surface) => surface.id === result.id)).toBeFalsy();
  expect(nativeSlot(result.hiddenLayout)?.children.includes(result.id)).toBeTruthy();
  expect(result.shownLayout.asides.some((surface) => surface.id === result.id)).toBeTruthy();
  expect(result.visibility.hide).toEqual([
    { id: result.id, kind: 'overlay', source: 'opener' },
  ]);
  expect(result.visibility.show).toEqual([
    { id: result.id, kind: 'overlay', source: 'opener' },
  ]);
  expect(result.closed).toEqual([
    { id: result.id, kind: 'overlay', reason: 'programmatic' },
  ]);
  expect(result.aliveAfterClose).toBeFalsy();
  expect(result.visibleAfterClose).toBeFalsy();
  expect(containsSurface(result.afterClose, result.id)).toBeFalsy();
  expect(result.afterRepeatedClose.mainSwitcher.revision).toBe(result.revisionAfterClose);
  expect(topology(result.afterRepeatedClose)).toEqual(topology(result.afterClose));
  expect(result.aside.mainSwitcher.revision < result.mainLayout.mainSwitcher.revision).toBeTruthy();
  expect(result.mainLayout.mainSwitcher.revision < result.dockedLayout.mainSwitcher.revision)
    .toBeTruthy();
  expect(topology(result.afterCleanup)).toEqual(topology(before));
});

desktopTest('switches, deduplicates concurrent opens, and leaves no ghost rows', async () => {
  const app = await desktopApp();
  const before = await app.surfaceLayout();
  const token = `automation-${Date.now()}-${Math.random().toString(36).slice(2)}`;
  const keys = {
    first: `${token}-first`,
    second: `${token}-second`,
    concurrent: `${token}-concurrent`,
  };
  const result = await app.eval({
    timeoutMs: 60_000,
    script: `
      const driver = lx.automation().lxapp();
      const snapshot = () => driver.surfaceLayout();
      const settle = () => new Promise((resolve) => setTimeout(resolve, 100));
      const waitFor = async (predicate, label) => {
        const deadline = Date.now() + 5_000;
        while (Date.now() < deadline) {
          if (predicate()) return;
          await new Promise((resolve) => setTimeout(resolve, 20));
        }
        throw new Error(label + ' was not observed');
      };
      const keys = ${JSON.stringify(keys)};
      const events = { firstHide: [], firstShow: [], secondHide: [], concurrentClose: [] };
      const opened = [];
      const off = [];
      let output;
      try {
        const baseline = await snapshot();
        const first = await lx.openSurface({
          surface: 'terminal', key: '  ' + keys.first + '  ', as: 'main',
        });
        opened.push(first);
        off.push(
          first.onHide((event) => events.firstHide.push(event)),
          first.onShow((event) => events.firstShow.push(event)),
        );
        const afterFirst = await snapshot();

        const second = await lx.openSurface({
          surface: 'terminal', key: keys.second, as: 'main',
        });
        opened.push(second);
        off.push(second.onHide((event) => events.secondHide.push(event)));
        await waitFor(() => events.firstHide.length === 1, 'first hide after second open');
        const afterSecond = await snapshot();

        const reopened = await lx.openSurface({
          surface: 'terminal', key: keys.first, as: 'main',
        });
        opened.push(reopened);
        await waitFor(
          () => events.firstShow.length === 1 && events.secondHide.length === 1,
          'paired show/hide after reopening first',
        );
        const afterReopen = await snapshot();

        const [concurrentFirst, concurrentSecond] = await Promise.all([
          lx.openSurface({ surface: 'terminal', key: keys.concurrent, as: 'main' }),
          lx.openSurface({ surface: 'terminal', key: keys.concurrent, as: 'main' }),
        ]);
        opened.push(concurrentFirst, concurrentSecond);
        off.push(concurrentFirst.onClose((event) => events.concurrentClose.push(event)));
        await waitFor(() => events.firstHide.length === 2, 'first hide after concurrent open');
        const afterConcurrent = await snapshot();

        await concurrentFirst.close();
        await waitFor(() => events.concurrentClose.length === 1, 'concurrent close');
        await settle();
        const afterClose = await snapshot();
        await concurrentSecond.close();
        await settle();
        const afterRepeatedClose = await snapshot();

        output = {
          baseline,
          ids: {
            first: first.id,
            second: second.id,
            concurrent: concurrentFirst.id,
          },
          firstState: {
            role: first.role,
            presentation: first.presentation,
            alive: first.alive,
          },
          distinctIds: first.id !== second.id && second.id !== concurrentFirst.id,
          reopenedSameId: reopened.id === first.id,
          reopenedSameHandle: reopened === first,
          concurrentSameId: concurrentFirst.id === concurrentSecond.id,
          concurrentSameHandle: concurrentFirst === concurrentSecond,
          concurrentAliveAfterClose: concurrentFirst.alive,
          concurrentVisibleAfterClose: concurrentFirst.visible,
          afterFirst,
          afterSecond,
          afterReopen,
          afterConcurrent,
          afterClose,
          afterRepeatedClose,
          events: {
            firstHide: events.firstHide.map((event) => ({ ...event })),
            firstShow: events.firstShow.map((event) => ({ ...event })),
            secondHide: events.secondHide.map((event) => ({ ...event })),
            concurrentClose: events.concurrentClose.map((event) => ({ ...event })),
          },
        };
      } finally {
        for (const unsubscribe of off.splice(0)) unsubscribe();
        for (const surface of [...new Set(opened)].reverse()) {
          if (surface.alive) await surface.close();
        }
      }
      output.afterCleanup = await snapshot();
      return output;
    `,
  }) as {
    baseline: SurfaceLayoutSnapshot;
    ids: { first: string; second: string; concurrent: string };
    firstState: { role: string; presentation: string; alive: boolean };
    distinctIds: boolean;
    reopenedSameId: boolean;
    reopenedSameHandle: boolean;
    concurrentSameId: boolean;
    concurrentSameHandle: boolean;
    concurrentAliveAfterClose: boolean;
    concurrentVisibleAfterClose: boolean;
    afterFirst: SurfaceLayoutSnapshot;
    afterSecond: SurfaceLayoutSnapshot;
    afterReopen: SurfaceLayoutSnapshot;
    afterConcurrent: SurfaceLayoutSnapshot;
    afterClose: SurfaceLayoutSnapshot;
    afterRepeatedClose: SurfaceLayoutSnapshot;
    afterCleanup: SurfaceLayoutSnapshot;
    events: {
      firstHide: VisibilityEvent[];
      firstShow: VisibilityEvent[];
      secondHide: VisibilityEvent[];
      concurrentClose: CloseEvent[];
    };
  };

  const baselineIds = switcherIds(result.baseline);
  expect(topology(result.baseline)).toEqual(topology(before));
  expect(result.firstState).toEqual({ role: 'main', presentation: 'main', alive: true });
  expect(result.distinctIds).toBeTruthy();
  expect(result.reopenedSameId).toBeTruthy();
  expect(result.reopenedSameHandle).toBeTruthy();
  expect(result.concurrentSameId).toBeTruthy();
  expect(result.concurrentSameHandle).toBeTruthy();
  expect(switcherIds(result.afterFirst)).toEqual([...baselineIds, result.ids.first]);
  expect(result.afterFirst.activeMainId).toBe(result.ids.first);
  expect(switcherIds(result.afterSecond)).toEqual([
    ...baselineIds, result.ids.first, result.ids.second,
  ]);
  expect(result.afterSecond.activeMainId).toBe(result.ids.second);
  expect(result.afterSecond.mainSwitcher.items.find((item) => (
    item.surfaceId === result.ids.first
  ))?.active).toBeFalsy();
  expect(switcherIds(result.afterReopen)).toEqual([
    ...baselineIds, result.ids.first, result.ids.second,
  ]);
  expect(result.afterReopen.activeMainId).toBe(result.ids.first);
  expect(switcherIds(result.afterConcurrent)).toEqual([
    ...baselineIds, result.ids.first, result.ids.second, result.ids.concurrent,
  ]);
  expect(result.afterConcurrent.mainSwitcher.items.filter((item) => (
    item.surfaceId === result.ids.concurrent
  )).length).toBe(1);
  expect(result.afterConcurrent.activeMainId).toBe(result.ids.concurrent);
  expect(switcherIds(result.afterClose)).toEqual([
    ...baselineIds, result.ids.first, result.ids.second,
  ]);
  expect(result.afterClose.activeMainId).toBe(result.ids.second);
  expect(result.concurrentAliveAfterClose).toBeFalsy();
  expect(result.concurrentVisibleAfterClose).toBeFalsy();
  expect(result.afterRepeatedClose.mainSwitcher.revision)
    .toBe(result.afterClose.mainSwitcher.revision);
  expect(topology(result.afterRepeatedClose)).toEqual(topology(result.afterClose));
  expect(result.events.firstHide).toEqual([
    { id: result.ids.first, kind: 'window', source: 'shell' },
    { id: result.ids.first, kind: 'window', source: 'shell' },
  ]);
  expect(result.events.firstShow).toEqual([
    { id: result.ids.first, kind: 'window', source: 'shell' },
  ]);
  expect(result.events.secondHide).toEqual([
    { id: result.ids.second, kind: 'window', source: 'shell' },
  ]);
  expect(result.events.concurrentClose).toEqual([
    { id: result.ids.concurrent, kind: 'window', reason: 'programmatic' },
  ]);
  expect(topology(result.afterCleanup)).toEqual(topology(before));
});
