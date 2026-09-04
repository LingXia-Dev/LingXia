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

function macosShowcaseHost(windows: DesktopWindowInfo[]): DesktopWindowInfo | undefined {
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

async function clickMacosStaticSettings(desktop: DesktopDriver): Promise<void> {
  const host = macosShowcaseHost(await desktop.windows());
  if (!host) throw new Error('visible macOS showcase host window was not found');
  await desktop.window.focus({ window: host.id });
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
  expect(settings.length).toBe(1);
  await desktop.ax.invoke({ window: host.id, match: `id:${settings[0].id}` });
}

async function openHostSettings(app: LxAppDriver, platform: string): Promise<void> {
  if (platform === 'macos') {
    await clickMacosStaticSettings(lx.automation().desktop);
    return;
  }
  await app.eval({
    timeoutMs: 20_000,
    script: `await lx.shell.openBuiltin('settings');`,
  });
}

spec("restore rendered home content after closing covering web tabs", { id: "DESKTOP-BROWSER-001", covers: [
      'BrowserDriver.tabs',
      'BrowserDriver.close',
      'lx.shell',
      'lx.shell.openBuiltin',
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

    await openHostSettings(app, platform);
    const settings = await eventually(
      () => browser.current(),
      (tab) => Boolean(tab?.current_url?.startsWith('lingxia://settings')),
      { describe: 'static Settings destination to open', timeoutMs: 15_000 },
    );
    if (!settings) throw new Error('static Settings destination did not produce a current tab');

    if (platform === 'macos') {
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
    }
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
