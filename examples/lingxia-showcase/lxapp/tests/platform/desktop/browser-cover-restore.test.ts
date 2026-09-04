import { waitForCurrentPage } from '../../helpers/page.js';
import { expect, spec } from '@lingxia/test';
import { bindFixture, eventually, specNamespace } from '../../helpers/poll.js';
import { SHOWCASE_APP_ID } from '../../helpers/app.js';
import { runtimePlatform } from '../../helpers/platform.js';
import type {
  DesktopDriver,
  DesktopWindowInfo,
  LxAppDriver,
} from 'lingxia-types/automation';

const WINDOWS_FOOTER_ACTION_COUNT = 5;

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

function windowsStaticSettingsPoint(
  host: DesktopWindowInfo,
  rail: boolean,
  footerActionCount: number,
): [number, number] {
  const scale = host.scale;
  const cell = 30 * scale;
  const margin = 6 * scale;
  if (!rail) {
    return [host.bounds.x + 92 * scale, host.bounds.y + host.bounds.h - margin - cell / 2];
  }
  const gap = 4 * scale;
  const expandCell = 34 * scale;
  const total = footerActionCount * cell + (footerActionCount - 1) * gap;
  const firstTop = host.bounds.h - gap - expandCell - margin - total;
  return [
    host.bounds.x + 22 * scale,
    host.bounds.y + firstTop
      + (footerActionCount - 1) * (cell + gap)
      + cell / 2,
  ];
}

async function clickStaticSettings(
  app: LxAppDriver,
  platform: string,
  desktop: DesktopDriver,
  footerActionCount = WINDOWS_FOOTER_ACTION_COUNT,
): Promise<void> {
  const host = desktopShowcaseHost(await desktop.windows());
  if (!host) throw new Error(`visible ${platform} showcase host window was not found`);
  await desktop.window.focus({ window: host.id });
  if (platform === 'windows') {
    const layout = await app.surfaceLayout();
    await desktop.pointer.click({
      at: windowsStaticSettingsPoint(
        host,
        layout.switcherForm === 'rail',
        footerActionCount,
      ),
    });
    return;
  }
  const buttons = await desktop.ax.query({
    window: host.id,
    match: 'name:Settings',
    all: true,
  });
  const settings = buttons.filter((node) => (
    node.role === 'button'
    && node.enabled
    && node.name.trim() === 'Settings'
    && node.rect.w > 0
    && node.rect.h > 0
    && node.rect.x < host.bounds.x + Math.min(220, host.bounds.w * 0.3)
  ));
  expect(settings.length).toBeGreaterThan(0);
  settings.sort((left, right) => left.rect.y - right.rect.y || left.rect.x - right.rect.x);
  const staticSettings = settings[settings.length - 1];
  await desktop.ax.invoke({ window: host.id, match: `id:${staticSettings.id}` });
}

async function openHostSettings(
  app: LxAppDriver,
  platform: string,
  footerActionCount = WINDOWS_FOOTER_ACTION_COUNT,
): Promise<void> {
  await clickStaticSettings(app, platform, lx.automation().desktop, footerActionCount);
}

async function restoreShowcaseSidebarActions(app: LxAppDriver): Promise<void> {
  await app.eval({
    script: `
      lx.shell.sidebarActions.replace([
        {
          id: 'downloads', placement: 'header', icon: 'public/sidebar-downloads.svg', label: 'Downloads',
          onActivate() { void lx.shell.openBuiltin('downloads'); },
        },
        {
          id: 'chat', placement: 'footer', icon: 'public/activator.svg', label: 'chat',
          onActivate() { void lx.surface.openDeclared('lingxia-chat'); },
        },
        {
          id: 'terminal-settings', placement: 'footer', icon: 'public/sidebar-terminal.svg', label: 'Terminal Settings',
          onActivate() { void lx.shell.openApp('app.lingxia.terminal-settings', { as: 'aside', edge: 'right' }); },
        },
        {
          id: 'terminal', placement: 'footer', icon: 'public/activator.svg', label: 'Terminal',
          onActivate() { void lx.surface.openDeclared('terminal'); },
        },
        {
          id: 'ping', placement: 'footer', icon: 'public/activator.svg', label: 'Ping',
          onActivate() { lx.showToast({ title: 'sidebar action clicked', icon: 'success' }); },
        },
      ]);
      delete globalThis.__staticSettingsSpoofCalls;
    `,
  });
}

spec("restore rendered home content after closing covering web tabs", { id: "DESKTOP-BROWSER-001", covers: [
      'BrowserDriver.tabs',
      'BrowserDriver.close',
      'DesktopDriver.pointer',
      'DesktopAx.invoke',
      'LxAppDriver.surfaceLayout',
    ], app: SHOWCASE_APP_ID }, async (t) => {
  const { app, defer } = bindFixture(t, "DESKTOP-BROWSER-001");

    const browser = lx.automation().browser;
    const platform = await runtimePlatform(app);
    const renderedBodyLength = async (): Promise<number> => Number(await app.page.eval({
      page: 'home',
      script: 'document.body ? document.body.innerText.length : -1',
    }));

    await app.nav.switchTab({ page: 'home' });
    await waitForCurrentPage(app, 'home');
    await app.page.waitFor({ page: 'home', css: '[data-testid="home-page"]', state: 'visible' });
    await eventually(renderedBodyLength, (length) => length > 0, {
      describe: 'baseline home page to render',
    });

    const tabsBefore = new Set((await browser.tabs()).map((tab) => tab.tab_id));
    defer(async () => {
      const freshTabs = (await browser.tabs()).filter((tab) => !tabsBefore.has(tab.tab_id));
      for (const tab of freshTabs) await browser.close({ tab: tab.tab_id });
    });
    defer(() => restoreShowcaseSidebarActions(app));

    await app.eval({
      script: `
        globalThis.__staticSettingsSpoofCalls = 0;
        const spoof = () => { globalThis.__staticSettingsSpoofCalls += 1; };
        lx.shell.sidebarActions.replace([
          { id: 'settings', placement: 'footer', icon: 'public/showcase-icon.svg', label: 'ID spoof', onActivate: spoof },
          { id: 'label-spoof', placement: 'footer', icon: 'public/showcase-icon.svg', label: 'Settings', onActivate: spoof },
          { id: 'icon-spoof', placement: 'footer', icon: 'public/sidebar-settings.svg', label: 'Icon spoof', onActivate: spoof },
        ]);
      `,
    });
    // Three runtime items plus the separately typed static item. Presentation
    // strings may match, but the static click must never dispatch their callbacks.
    await openHostSettings(app, platform, 4);
    const settings = await eventually(
      () => browser.current(),
      (tab) => Boolean(tab?.current_url?.startsWith('lingxia://settings')),
      { describe: 'static Settings destination to open', timeoutMs: 15_000 },
    );
    if (!settings) throw new Error('static Settings destination did not produce a current tab');
    expect(await app.eval({ script: `return globalThis.__staticSettingsSpoofCalls;` })).toBe(0);
    await restoreShowcaseSidebarActions(app);

    await browser.eval({
      tab: settings.tab_id,
      js: `globalThis.__staticSettingsReloadProbe = 'stale';`,
    });
    await openHostSettings(app, platform);
    await eventually(
      () => browser.eval({
        tab: settings.tab_id,
        js: `typeof globalThis.__staticSettingsReloadProbe`,
      }),
      (value) => value === 'undefined',
      { describe: 'same Settings target to receive a fresh trusted reload', timeoutMs: 15_000 },
    );
    expect((await browser.current())?.tab_id).toBe(settings.tab_id);

    await browser.open({ url: 'about:blank', tab: settings.tab_id });
    await eventually(
      () => browser.current(),
      (tab) => tab?.current_url === 'about:blank',
      { describe: 'external navigation away from Settings', timeoutMs: 15_000 },
    );
    await openHostSettings(app, platform);
    const restored = await eventually(
      () => browser.current(),
      (tab) => Boolean(tab?.current_url?.startsWith('lingxia://settings')),
      { describe: 'static Settings destination to restore trusted authority', timeoutMs: 15_000 },
    );
    if (!restored) throw new Error('static Settings destination did not restore a current tab');
    expect(restored.tab_id).toBe(settings.tab_id);
    await app.eval({
      timeoutMs: 20_000,
      script: `await lx.shell.openBuiltin('downloads');`,
    });

    const opened = await eventually(
      async () => (await browser.tabs()).filter((tab) => !tabsBefore.has(tab.tab_id)),
      (tabs) => tabs.length >= 2,
      { describe: 'two covering web tabs to open', timeoutMs: 15_000 });

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
    expect(layout.mains.includes('lingxia-showcase')).toBeTruthy();

    const after = await eventually(renderedBodyLength, (length) => length > 0, {
      describe: 'restored home page to have rendered body text',
      timeoutMs: 20_000,
    });
    expect(after).toBeGreaterThan(0);
  });
