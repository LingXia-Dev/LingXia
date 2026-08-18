import { waitForCurrentPage } from '../../helpers/page.js';
import { expect, spec } from '@lingxia/test';
import { bindFixture, eventually, specNamespace } from '../../helpers/poll.js';
import { SHOWCASE_APP_ID } from '../../helpers/app.js';

spec("restore rendered home content after closing covering web tabs", { id: "DESKTOP-BROWSER-001", covers: [
      'BrowserDriver.tabs',
      'BrowserDriver.close',
      'lx.shell',
      'lx.shell.openBuiltin',
      'LxAppDriver.surfaceLayout',
    ], app: SHOWCASE_APP_ID }, async (t) => {
  const { app, defer } = bindFixture(t, "DESKTOP-BROWSER-001");

    const browser = lx.automation().browser;
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

    await app.eval({
      timeoutMs: 20_000,
      script: `await lx.shell.openBuiltin('settings');`,
    });
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
    expect(after > 0).toBeTruthy();
  });
