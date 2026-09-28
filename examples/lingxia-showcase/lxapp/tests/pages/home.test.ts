import type { TestApp } from '@lingxia/test';
import { expect, spec } from '@lingxia/test';
import type { LxAppRuntimeTabBarInfo } from '@lingxia/types/automation';
import { SHOWCASE_APP_ID } from '../helpers/app.js';
import { eventually } from '../helpers/poll.js';
import {
  currentPageOrNull,
  waitForCurrentPage,
  waitForCurrentPageVisible,
  waitForElementAttribute,
  waitForElementEnabled,
  waitForElementText,
} from '../helpers/page.js';

async function waitForTabBar(
  app: TestApp,
  accept: (state: LxAppRuntimeTabBarInfo) => boolean,
  describe: string,
): Promise<LxAppRuntimeTabBarInfo> {
  return eventually(
    async () => {
      const state = (await app.info()).tabBar;
      if (state === null) throw new Error('showcase TabBar is not declared');
      return state;
    },
    accept,
    { describe, timeoutMs: 15_000, retryIf: () => true },
  );
}

spec('home View MessagePort is up without relaunch', async (t) => {
  const app = t.automation.lxapp(SHOWCASE_APP_ID);
  const current = await currentPageOrNull(app);
  if (current?.name !== 'home') {
    await app.nav.switchTab({ page: 'home' });
  }
  // `ready` waits on View handshake. Do not relaunch: that remounts a visible
  // WebView and hides the cold-start MessagePort miss.
  await waitForCurrentPage(app, 'home', 20_000);
});

spec('greets through real page input and the Logic bridge', async (t) => {
  const app = t.automation.lxapp(SHOWCASE_APP_ID);
  await app.nav.relaunch({ page: 'home' });
  await waitForCurrentPageVisible(app, 'home', '[data-testid="home-page"]');
  await eventually(
    () => app.logic.eval({ timeout: 20_000 }, ({ lx }) => typeof lx.host.displayLanguage.get),
    (kind) => kind === 'function',
    { describe: 'home Logic runtime', timeoutMs: 20_000, retryIf: () => true },
  );

  const home = await app.page({ name: 'home' });
  const name = `Gate ${Date.now()}`;
  await home.view.testId('home-name').fill(name);
  await waitForElementAttribute(
    home.view,
    '[data-testid="home-name"]',
    'data-controlled-value',
    name,
  );
  await waitForElementEnabled(home.view, '[data-testid="home-greet"]');
  await home.view.testId('home-greet').click();

  expect(await waitForElementText(
    t,
    home.view,
    '[data-testid="home-greeting"]',
    (text) => text.includes(name),
    30_000,
  )).toContain(name);
});

spec('switches display language from the home control', {
  id: 'UI-LANGUAGE-001',
  covers: [
    'lx.host.displayLanguage.watch',
    'lx.host.control.displayLanguage.getPreference',
    'lx.host.control.displayLanguage.setPreference',
    'lx.host.control.displayLanguage.watchPreference',
  ],
  app: SHOWCASE_APP_ID,
  timeout: 60_000,
}, async (t) => {
  const app = t.automation.lxapp(SHOWCASE_APP_ID);
  await app.nav.relaunch({ page: 'home' });
  await waitForCurrentPageVisible(app, 'home', '[data-testid="home-language"]');
  await eventually(
    () => app.logic.eval({ timeout: 20_000 }, ({ lx }) => typeof lx.host.control?.displayLanguage?.setPreference),
    (kind) => kind === 'function',
    { describe: 'home Logic displayLanguage control', timeoutMs: 20_000, retryIf: () => true },
  );

  const original = await app.logic.eval(({ lx }) => {
    const control = lx.host.control;
    if (!control) throw new Error('lx.host.control is not injected');
    return control.displayLanguage.getPreference();
  });

  // Switching the language re-renders home in place; it does not remount it.
  const home = await app.page({ name: 'home' });
  try {
    await waitForElementEnabled(home.view, '[data-testid="home-language-zh-CN"]');
    await home.view.testId('home-language-zh-CN').click();
    await eventually(
      () => app.logic.eval({ timeout: 15_000 }, ({ lx }) => lx.host.control?.displayLanguage.getPreference()),
      (preference) => preference === 'zh-CN',
      {
        describe: 'home language control to set zh-CN',
        timeoutMs: 15_000,
        intervalMs: 400,
      },
    );
    expect(await waitForElementText(
      t,
      home.view,
      '[data-testid="home-tagline"]',
      (text) => text.includes('轻量应用框架'),
      15_000,
    )).toContain('轻量应用框架');
    await waitForElementAttribute(
      home.view,
      '[data-testid="home-language-zh-CN"]',
      'data-selected',
      'true',
    );
    await waitForTabBar(
      app,
      (state) =>
        state.items[0]?.text === '首页' &&
        state.items[1]?.text === '接口' &&
        state.items[2]?.text === '组件' &&
        state.items[3]?.text === '待办',
      'tab bar labels after zh-CN',
    );

    await waitForElementEnabled(home.view, '[data-testid="home-language-en-US"]');
    await home.view.testId('home-language-en-US').click();
    await eventually(
      () => app.logic.eval({ timeout: 15_000 }, ({ lx }) => lx.host.control?.displayLanguage.getPreference()),
      (preference) => preference === 'en-US',
      {
        describe: 'home language control to set en-US',
        timeoutMs: 15_000,
        intervalMs: 400,
      },
    );
    expect(await waitForElementText(
      t,
      home.view,
      '[data-testid="home-tagline"]',
      (text) => text.includes('Lightweight Application Framework'),
      15_000,
    )).toContain('Lightweight Application Framework');
    await waitForElementAttribute(
      home.view,
      '[data-testid="home-language-en-US"]',
      'data-selected',
      'true',
    );
    await waitForTabBar(
      app,
      (state) =>
        state.items[0]?.text === 'Home' &&
        state.items[1]?.text === 'API' &&
        state.items[2]?.text === 'Components' &&
        state.items[3]?.text === 'ToDo',
      'tab bar labels after en-US',
    );
  } finally {
    await app.logic.eval(async ({ lx }, original) => {
      await lx.host.control?.displayLanguage.setPreference(original);
    }, original);
  }
});
