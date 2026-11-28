import { waitForCurrentPage } from '@lingxia/test/showcase/helpers/page.js';
import { notDesktop } from './only.js';
import { expect, test, type Fixture } from '@lingxia/test';
import { bindFixture, eventually, specNamespace } from '@lingxia/test/showcase/helpers/poll.js';
import { SHOWCASE_APP_ID } from '@lingxia/test/showcase/helpers/app.js';
import { runtimePlatform } from '@lingxia/test/showcase/helpers/platform.js';
import type { DesktopDriver, DesktopWindowInfo } from '@lingxia/types/automation';
import type { TestApp } from '@lingxia/test';

test.configure({ skip: notDesktop });

/// Showcase sidebar actions besides the host's typed Settings: Downloads in
/// the header, five footer actions.
const SHOWCASE_SIDEBAR_ACTIONS: SidebarActionCounts = { header: 1, footer: 5 };

interface SidebarActionCounts {
  /** Runtime header actions; the typed Settings leads them. */
  header: number;
  footer: number;
}

function desktopShowcaseHost(windows: DesktopWindowInfo[]): DesktopWindowInfo | undefined {
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

function windowContains(outer: DesktopWindowInfo, inner: DesktopWindowInfo): boolean {
  const tolerance = 8;
  return inner.bounds.x >= outer.bounds.x - tolerance
    && inner.bounds.y >= outer.bounds.y - tolerance
    && inner.bounds.x + inner.bounds.w <= outer.bounds.x + outer.bounds.w + tolerance
    && inner.bounds.y + inner.bounds.h <= outer.bounds.y + outer.bounds.h + tolerance;
}

function windowsSidebarUsesRail(
  host: DesktopWindowInfo,
  windows: DesktopWindowInfo[],
): boolean {
  const main = windows
    .filter((window) => (
      window.visible
      && window.process.toLocaleLowerCase() === 'msedgewebview2'
      && windowContains(host, window)
      && window.bounds.w > 0
      && window.bounds.h > 0
    ))
    .sort((left, right) => right.bounds.w * right.bounds.h - left.bounds.w * left.bounds.h)[0];
  if (!main) throw new Error('visible Windows host WebView was not found');
  // SurfaceLayout.switcherForm describes the surface graph, not the shell's
  // persisted 44-DIP icon rail. The physical content inset distinguishes it
  // from the expanded sidebar.
  return main.bounds.x - host.bounds.x < 80 * host.scale;
}

function windowsStaticSettingsPoint(
  host: DesktopWindowInfo,
  rail: boolean,
  actions: SidebarActionCounts,
): [number, number] {
  const scale = host.scale;
  if (!rail) {
    // Typed Settings leads the header actions, which sit right-aligned
    // against the collapse toggle at the expanded sidebar's trailing edge
    // (lingxia-shell DEFAULT_EXPANDED_SIDEBAR_WIDTH).
    const size = 24;
    const gap = 4;
    const toggleLeft = 148 - 8 - size;
    const shown = Math.min(1 + actions.header, 2);
    const settingsLeft = toggleLeft - gap - shown * size - (shown - 1) * gap;
    return [
      host.bounds.x + (settingsLeft + size / 2) * scale,
      host.bounds.y + 16 * scale,
    ];
  }
  // The rail has no header row: header actions lead its action stack, so
  // Settings is the first cell. At most five cells show.
  const cell = 30 * scale;
  const gap = 4 * scale;
  const margin = 8 * scale;
  const visible = Math.min(1 + actions.header + actions.footer, 5);
  const total = visible * cell + (visible - 1) * gap;
  const firstTop = host.bounds.h - margin - total;
  return [
    host.bounds.x + 22 * scale,
    host.bounds.y + firstTop + cell / 2,
  ];
}

/** The spoofed sidebar actions count their calls on Logic's `globalThis`. */
type SpoofGlobals = { __staticSettingsSpoofCalls?: number };

async function clickStaticSettings(
  app: TestApp,
  platform: string,
  desktop: DesktopDriver,
  actions = SHOWCASE_SIDEBAR_ACTIONS,
): Promise<void> {
  const windows = await desktop.windows();
  const host = desktopShowcaseHost(windows);
  if (!host) throw new Error(`visible ${platform} showcase host window was not found`);
  await desktop.window.focus({ window: host.id });
  if (platform === 'windows') {
    // Retain the public layout contract coverage while deriving native chrome
    // geometry from the window tree that the pointer actually targets.
    await app.surfaceLayout();
    await desktop.pointer.click({
      at: windowsStaticSettingsPoint(
        host,
        windowsSidebarUsesRail(host, windows),
        actions,
      ),
    });
    return;
  }
  const buttons = await desktop.ax.query({
    window: host.id,
    match: 'role:button',
    all: true,
  });
  const settings = buttons.filter((node) => (
    node.role === 'button'
    && node.enabled
    && (node.name.trim() === 'Settings' || node.name.trim() === '设置')
    && node.rect.w > 0
    && node.rect.h > 0
    && node.rect.x < host.bounds.x + Math.min(220, host.bounds.w * 0.3)
  ));
  expect(settings.length).toBeGreaterThan(0);
  // Typed Settings is header chrome (or the rail's first action), above any
  // runtime footer action that borrows its label.
  settings.sort((left, right) => left.rect.y - right.rect.y || left.rect.x - right.rect.x);
  const staticSettings = settings[0];
  await desktop.ax.invoke({ window: host.id, match: `id:${staticSettings.id}` });
}

async function openHostSettings(
  t: Fixture,
  app: TestApp,
  platform: string,
  actions = SHOWCASE_SIDEBAR_ACTIONS,
): Promise<void> {
  await clickStaticSettings(app, platform, t.automation.desktop, actions);
}

async function restoreShowcaseSidebarActions(app: TestApp): Promise<void> {
  await app.logic.eval(async ({ lx }) => {
    await lx.shell.sidebarActions.replace([
      {
        id: 'downloads', placement: 'header', icon: 'public/sidebar-downloads.svg', label: 'Downloads',
        onSelect() { void lx.surface.open({ app: 'app.lingxia.browser', page: 'downloads' }); },
      },
      {
        id: 'chat', placement: 'footer', icon: 'public/sidebar-chat.svg', label: 'chat',
        onSelect() { void lx.surface.open({ declared: 'lingxia-showcase-web' }); },
      },
      {
        id: 'terminal-settings', placement: 'footer', icon: 'public/sidebar-terminal-settings.svg', label: 'Terminal Settings',
        onSelect() { void lx.surface.open({ app: 'app.lingxia.terminal-settings' }, { as: 'aside', edge: 'right' }); },
      },
      {
        id: 'terminal', placement: 'footer', icon: 'public/sidebar-terminal.svg', label: 'Terminal',
        onSelect() { void lx.surface.open({ declared: 'terminal' }); },
      },
      {
        id: 'ping', placement: 'footer', icon: 'public/activator.svg', label: 'Ping',
        onSelect() { lx.ui.toast('sidebar action clicked', { tone: 'success' }); },
      },
      {
        id: 'language', placement: 'footer', icon: 'public/sidebar-language.svg', label: 'Language', value: 'English',
        onSelect() {},
      },
    ]);
    delete (globalThis as SpoofGlobals).__staticSettingsSpoofCalls;
  });
}

/** The browser's system pages: a host-bundled lxapp, opened as a control surface. */
const BROWSER_APP_ID = 'app.lingxia.browser';

/** The browser lxapp's live session and page, or `undefined` while it is not open. */
async function browserSystemApp(t: Fixture): Promise<{ sessionId: number; page: string } | undefined> {
  const info = (await t.automation.lxapps.list()).find((app) => app.appId === BROWSER_APP_ID);
  if (!info || info.status === 'closed' || !info.currentPage) return undefined;
  return { sessionId: info.sessionId, page: info.currentPage };
}

async function waitForBrowserSystemPage(
  t: Fixture,
  app: TestApp,
  page: string,
  describe: string,
): Promise<{ sessionId: number; page: string }> {
  const opened = await eventually(
    () => browserSystemApp(t),
    (state) => Boolean(state?.page.startsWith(`pages/${page}/index`)),
    { describe, timeoutMs: 15_000 },
  );
  if (!opened) throw new Error(`${describe}: ${BROWSER_APP_ID} did not open`);
  const layout = await app.surfaceLayout();
  expect(Object.values(layout.identities).some((identity) => identity.appId === BROWSER_APP_ID)).toBeTruthy();
  return opened;
}

test("restore rendered home content after closing covering web tabs", { id: "DESKTOP-BROWSER-001", covers: [
      'BrowserDriver.tabs',
      'BrowserDriver.close',
      'DesktopDriver.pointer',
      'DesktopAx.invoke',
      'LxAppDriver.surfaceLayout',
      'lx.surface.open',
    ], app: SHOWCASE_APP_ID }, async (t) => {
  const { app, defer } = bindFixture(t, "DESKTOP-BROWSER-001");

    const browser = t.automation.browser;
    const platform = await runtimePlatform(app);

    await app.nav.go({ tab: 'home' });
    await waitForCurrentPage(app, 'home');
    // Web tabs cover this home instance and closing them uncovers the same one.
    const home = await app.page({ name: 'home' });
    // Renderer-agnostic: the home card's text, read through the page's locators.
    const renderedBodyLength = async (): Promise<number> => (
      (await home.view.testId('home-card').textContent()).trim().length
    );
    await home.view.testId('home-page').waitFor({ state: 'visible', timeout: 30_000 });
    await eventually(renderedBodyLength, (length) => length > 0, {
      describe: 'baseline home page to render',
    });

    const tabsBefore = new Set((await browser.tabs()).map((tab) => tab.tab_id));
    defer(async () => {
      const freshTabs = (await browser.tabs()).filter((tab) => !tabsBefore.has(tab.tab_id));
      for (const tab of freshTabs) await browser.close({ tab: tab.tab_id });
    });
    defer(async () => {
      if (await browserSystemApp(t)) await t.automation.lxapps.close({ app: BROWSER_APP_ID });
    });
    defer(() => restoreShowcaseSidebarActions(app));

    await app.logic.eval(async ({ lx }) => {
      const probe = globalThis as SpoofGlobals;
      probe.__staticSettingsSpoofCalls = 0;
      const spoof = () => { probe.__staticSettingsSpoofCalls = (probe.__staticSettingsSpoofCalls ?? 0) + 1; };
      await lx.shell.sidebarActions.replace([
        { id: 'settings', placement: 'footer', icon: 'public/showcase-icon.svg', label: 'ID spoof', onSelect: spoof },
        { id: 'label-spoof', placement: 'footer', icon: 'public/showcase-icon.svg', label: 'Settings', onSelect: spoof },
        { id: 'icon-spoof', placement: 'footer', icon: 'public/sidebar-settings.svg', label: 'Icon spoof', onSelect: spoof },
      ]);
    });
    // Three runtime footer items beside the separately typed static item.
    // Presentation strings may match, but the static click must never
    // dispatch their callbacks.
    await openHostSettings(t, app, platform, { header: 0, footer: 3 });
    const settings = await waitForBrowserSystemPage(t, app, 'settings', 'static Settings destination to open');
    expect(await app.logic.eval(() => (globalThis as SpoofGlobals).__staticSettingsSpoofCalls)).toBe(0);
    await restoreShowcaseSidebarActions(app);

    // A second click brings back the same session rather than starting another.
    await openHostSettings(t, app, platform);
    const again = await waitForBrowserSystemPage(t, app, 'settings', 'static Settings destination to reopen');
    expect(again.sessionId).toBe(settings.sessionId);

    await app.logic.eval({ timeout: 20_000 }, async ({ lx }, appId) => {
      await lx.surface.open({ app: appId, page: 'downloads' });
    }, BROWSER_APP_ID);
    const downloads = await waitForBrowserSystemPage(t, app, 'downloads', 'Downloads page to open');
    expect(downloads.sessionId).toBe(settings.sessionId);

    await t.automation.lxapps.close({ app: BROWSER_APP_ID });
    await eventually(() => browserSystemApp(t), (state) => state === undefined, {
      describe: 'browser system pages to close',
      timeoutMs: 15_000,
    });

    for (let index = 0; index < 2; index += 1) await browser.open({ url: 'about:blank' });
    const opened = await eventually(
      async () => (await browser.tabs()).filter((tab) => !tabsBefore.has(tab.tab_id)),
      (tabs) => tabs.length >= 2,
      { describe: 'covering web tabs to open', timeoutMs: 15_000 });

    for (const tab of opened) {
      await browser.close({ tab: tab.tab_id });
    }

    await eventually(
      async () => (await browser.tabs()).filter((tab) => !tabsBefore.has(tab.tab_id)),
      (tabs) => tabs.length === 0,
      { describe: 'covering web tabs to close', timeoutMs: 15_000 });

    const layout = await eventually(
      () => app.surfaceLayout(),
      (snapshot) => snapshot.activeMainId === snapshot.mainSwitcher.rootSurfaceId,
      { describe: 'root main to become active after closes', timeoutMs: 15_000 });
    expect(layout.mains.includes(SHOWCASE_APP_ID)).toBeTruthy();

    const after = await eventually(renderedBodyLength, (length) => length > 0, {
      describe: 'restored home page to have rendered body text',
      timeoutMs: 20_000,
    });
    expect(after).toBeGreaterThan(0);
  });
