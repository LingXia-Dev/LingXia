import { expect, spec } from '@lingxia/test';
import type { LxAppDriver, LxAppRuntimeTabBarInfo } from '@lingxia/types/automation';
import { SHOWCASE_APP_ID, showcaseApp } from '../helpers/app.js';
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
  app: LxAppDriver,
  accept: (state: LxAppRuntimeTabBarInfo) => boolean,
  describe: string,
): Promise<LxAppRuntimeTabBarInfo> {
  return eventually(
    async () => {
      const state = (await app.info()).tab_bar;
      if (state === null) throw new Error('showcase TabBar is not declared');
      return state;
    },
    accept,
    { describe, timeoutMs: 15_000, retryIf: () => true },
  );
}

spec('home View MessagePort is up without relaunch', async () => {
  const app = showcaseApp();
  const current = await currentPageOrNull(app);
  if (current?.name !== 'home') {
    await app.nav.switchTab({ page: 'home' });
  }
  // `ready` waits on View handshake. Do not relaunch: that remounts a visible
  // WebView and hides the cold-start MessagePort miss.
  await waitForCurrentPage(app, 'home', 20_000);
});

spec('greets through real page input and the Logic bridge', async () => {
  const app = showcaseApp();
  await app.nav.relaunch({ page: 'home' });
  await waitForCurrentPageVisible(app, 'home', '[data-testid="home-page"]');
  await eventually(
    () => app.eval({ script: 'return typeof lx.app.displayLanguage.get' }),
    (kind) => kind === 'function',
    { describe: 'home Logic runtime', timeoutMs: 20_000, retryIf: () => true },
  );

  const name = `Gate ${Date.now()}`;
  await app.page.fill({ page: 'home', css: '[data-testid="home-name"]', text: name });
  await waitForElementAttribute(
    app,
    'home',
    '[data-testid="home-name"]',
    'data-controlled-value',
    name,
  );
  await waitForElementEnabled(app, 'home', '[data-testid="home-greet"]');
  await app.page.click({ page: 'home', css: '[data-testid="home-greet"]' });

  expect(await waitForElementText(
    app,
    'home',
    '[data-testid="home-greeting"]',
    (text) => text.includes(name),
    30_000,
  )).toContain(name);
});

spec('switches display language from the home control', {
  id: 'UI-LANGUAGE-001',
  covers: [
    'lx.app.displayLanguage.watch',
    'lx.app.control.displayLanguage.getPreference',
    'lx.app.control.displayLanguage.setPreference',
    'lx.app.control.displayLanguage.watchPreference',
  ],
  app: SHOWCASE_APP_ID,
  timeout: 60_000,
}, async () => {
  const app = showcaseApp();
  await app.nav.relaunch({ page: 'home' });
  await waitForCurrentPageVisible(app, 'home', '[data-testid="home-language"]');
  await eventually(
    () => app.eval({ script: 'return typeof lx.app.control?.displayLanguage?.setPreference' }),
    (kind) => kind === 'function',
    { describe: 'home Logic displayLanguage control', timeoutMs: 20_000, retryIf: () => true },
  );

  const original = await app.eval({
    script: 'return lx.app.control.displayLanguage.getPreference()',
  }) as string;

  try {
    await app.page.scrollTo({ page: 'home', css: '[data-testid="home-language-zh-CN"]' });
    await waitForElementEnabled(app, 'home', '[data-testid="home-language-zh-CN"]');
    await eventually(
      async () => {
        await app.page.click({ page: 'home', css: '[data-testid="home-language-zh-CN"]' });
        return await app.eval({
          script: 'return lx.app.control.displayLanguage.getPreference()',
        });
      },
      (preference) => preference === 'zh-CN',
      {
        describe: 'home language control to set zh-CN',
        timeoutMs: 15_000,
        intervalMs: 400,
      },
    );
    expect(await waitForElementText(
      app,
      'home',
      '[data-testid="home-tagline"]',
      (text) => text.includes('轻量应用框架'),
      15_000,
    )).toContain('轻量应用框架');
    await waitForElementAttribute(
      app,
      'home',
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

    await app.page.scrollTo({ page: 'home', css: '[data-testid="home-language-en-US"]' });
    await waitForElementEnabled(app, 'home', '[data-testid="home-language-en-US"]');
    await eventually(
      async () => {
        await app.page.click({ page: 'home', css: '[data-testid="home-language-en-US"]' });
        return await app.eval({
          script: 'return lx.app.control.displayLanguage.getPreference()',
        });
      },
      (preference) => preference === 'en-US',
      {
        describe: 'home language control to set en-US',
        timeoutMs: 15_000,
        intervalMs: 400,
      },
    );
    expect(await waitForElementText(
      app,
      'home',
      '[data-testid="home-tagline"]',
      (text) => text.includes('Lightweight Application Framework'),
      15_000,
    )).toContain('Lightweight Application Framework');
    await waitForElementAttribute(
      app,
      'home',
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
    await app.eval({
      script: `await lx.app.control.displayLanguage.setPreference(${JSON.stringify(original)})`,
    });
  }
});
