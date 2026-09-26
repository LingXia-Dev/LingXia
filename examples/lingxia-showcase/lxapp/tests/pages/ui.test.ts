import { expect, spec } from '@lingxia/test';
import type { LxAppRuntimeTabBarInfo } from '@lingxia/types/automation';
import { waitForElementAttribute, waitForCurrentPage } from '../helpers/page.js';
import { bindFixture, evalCaught, eventually, specNamespace } from '../helpers/poll.js';
import { SHOWCASE_APP_ID } from '../helpers/app.js';

spec("run navigation APIs from the rendered UI controls", { id: "UI-NAV-001", covers: ['lx.navigateTo', 'lx.navigateBack', 'lx.redirectTo', 'lx.switchTab'], app: SHOWCASE_APP_ID }, async (t) => {
  const { app } = bindFixture(t, "UI-NAV-001");

  await app.nav.relaunch({ page: 'ui', query: { type: 'navigation' } });
  await app.page.waitFor({ page: 'ui', css: '[data-testid="ui-navigate-to"]', state: 'visible' });

  await app.view.testId("ui-navigate-to", { page: 'ui' }).click();
  await eventually(() => app.nav.stack(), (stack) => stack.length === 2, {
    describe: 'UI navigateTo to push a second page instance',
  });

  // The push created a fresh instance of this same route; wait for its
  // document before driving the next control.
  await app.page.waitFor({ page: 'ui', css: '[data-testid="ui-navigate-back"]', state: 'visible' });
  await app.view.testId("ui-navigate-back", { page: 'ui' }).click();
  await eventually(() => app.nav.stack(), (stack) => stack.length === 1, {
    describe: 'UI navigateBack to pop the page instance',
  });

  const readCurrentUiLifecycle = () => app.eval({
    script: `
      (() => {
        const data = getCurrentPages().find((page) => page.route.includes('/ui/'))?.data;
        return {
          instanceTag: data?.instanceTag ?? '',
          onLoadCount: (data?.events ?? []).filter((event) => event.endsWith('onLoad')).length,
        };
      })()
    `,
  }) as Promise<{ instanceTag: string; onLoadCount: number }>;
  await eventually(
    readCurrentUiLifecycle,
    ({ onLoadCount }) => onLoadCount > 0,
    { describe: 'current UI page onLoad count before redirect' },
  );
  await app.eval({
    script: `
      const page = getCurrentPages().find((candidate) => candidate.route.includes('/ui/'));
      if (!page) throw new Error('current UI PageInstance is missing');
      page.data.events = [];
    `,
  });
  await app.view.testId("ui-redirect-to", { page: 'ui' }).click();
  await eventually(() => app.nav.stack(), (stack) => stack.length === 1 && stack[0]?.name === 'ui', {
    describe: 'UI redirectTo to replace the current page',
  });
  // Rendered controls dispatch Logic actions as fire-and-forget notifications.
  // Observe the redirect in Logic, then wait for that exact instance tag to
  // reach the rendered document before sending the next UI intent.
  const redirected = await eventually(
    readCurrentUiLifecycle,
    ({ instanceTag, onLoadCount }) => instanceTag !== '' && onLoadCount > 0,
    { describe: 'same-route redirect onLoad lifecycle event' },
  );
  await waitForElementAttribute(
    app,
    'ui',
    '[data-testid="ui-page"]',
    'data-instance-tag',
    redirected.instanceTag,
  );

  await app.view.testId("ui-switch-tab", { page: 'ui' }).click();
  await waitForCurrentPage(app, 'home');
  expect((await app.nav.stack()).map(({ name }) => name)).toEqual(['home']);
});

spec("apply TabBar visibility, style, item, icon, badge, and red-dot updates", { id: "UI-TABBAR-001", covers: ['lx.tabBar', 'lx.tabBar.update'], app: SHOWCASE_APP_ID, timeout: 60_000 }, async (t) => {
  const { app, defer } = bindFixture(t, "UI-TABBAR-001");

  const tabBar = async (): Promise<LxAppRuntimeTabBarInfo> => {
    const state = (await app.info()).tab_bar;
    if (state === null) throw new Error('showcase TabBar is not declared');
    return state;
  };
  const waitForTabBar = (
    accept: (state: LxAppRuntimeTabBarInfo) => boolean,
    describe: string,
  ) => eventually(tabBar, accept, { describe });

  defer(async () => {
    await app.eval({
      script: `
        await lx.tabBar.update({
          visibility: 'auto',
          items: [{
            index: 1,
            text: null,
            iconPath: null,
            badge: null,
            redDot: false,
          }],
        });
      `,
    });
    await app.nav.relaunch({ page: 'home' });
  });

  await app.nav.relaunch({ page: 'ui', query: { type: 'tabbar' } });
  await app.page.waitFor({ page: 'ui', css: '[data-testid="tabbar-show"]', state: 'visible' });

  const automaticDetail = await waitForTabBar(
    ({ visibility, route_visible, effective_visible }) => (
      visibility === 'auto' && !route_visible && !effective_visible
    ),
    'automatic TabBar visibility on a non-tab route',
  );
  expect(automaticDetail.selected_index).toBe(-1);

  await app.view.testId("tabbar-show", { page: 'ui' }).click();
  const forced = await waitForTabBar(
    ({ visibility, route_visible, effective_visible }) => (
      visibility === 'visible' && !route_visible && effective_visible
    ),
    'forced TabBar visibility on a non-tab route',
  );
  expect(forced.effective_visible).toBeTruthy();

  await app.eval({
    script: `
      await lx.tabBar.update({
        items: [{
          index: 1,
          text: 'Automation',
          iconPath: 'public/home.png',
          badge: '7',
        }],
      });
    `,
  });
  // The runtime resolves relative icon paths against the package directory
  // with native separators, so Windows reports them with backslashes.
  const assetPath = (value: string | null | undefined) => (value ?? '').replace(/\\/g, '/');
  const styled = await waitForTabBar(
    (state) => (
      state.items[1]?.text === 'Automation'
      && assetPath(state.items[1]?.icon_path).endsWith('/public/home.png')
      && state.items[1]?.badge === '7'
      && state.items[1]?.red_dot === false
    ),
    'TabBar text and badge update',
  );

  const invalid = await evalCaught(
    app,
    `await lx.tabBar.update({ visibility: 'hidden', items: [{ index: 99, text: 'Invalid' }] });`,
  );
  expect(invalid.ok).toBeFalsy();
  expect(invalid.code).toBe('E_INVALID_ARG');
  expect(await tabBar()).toEqual(styled);

  await app.eval({
    script: `
      await lx.tabBar.update({
        items: [{ index: 1, badge: null, redDot: true }],
      });
    `,
  });
  await waitForTabBar(
    (state) => state.items[1]?.badge === null && state.items[1]?.red_dot === true,
    'TabBar badge replacement by a red dot',
  );

  await app.view.testId("tabbar-hide", { page: 'ui' }).click();
  await waitForTabBar(
    ({ visibility, effective_visible }) => visibility === 'hidden' && !effective_visible,
    'explicitly hidden TabBar',
  );

  await app.eval({ script: `await lx.tabBar.update({ visibility: 'auto' });` });
  await waitForTabBar(
    ({ visibility, route_visible, effective_visible }) => (
      visibility === 'auto' && !route_visible && !effective_visible
    ),
    'restored automatic visibility on a non-tab route',
  );

  await app.nav.relaunch({ page: 'home' });
  await app.page.waitFor({ page: 'home', css: 'body', state: 'attached' });
  await waitForTabBar(
    ({ visibility, route_visible, effective_visible, selected_index }) => (
      visibility === 'auto' && route_visible && effective_visible && selected_index === 0
    ),
    'automatic visibility after entering a tab route',
  );

  // Keep this an expression: Windows page eval runs the raw script through
  // ExecuteScript, where a top-level `return` is a syntax error.
  const readHomeViewportHeight = () => app.page.eval({
    page: 'home',
    script: 'window.innerHeight',
  }) as Promise<number>;
  const viewportBeforeChromeRefresh = await eventually(
    readHomeViewportHeight,
    (height) => height > 0,
    { describe: 'home WebView to expose a non-zero viewport' });
  await app.eval({
    script: `
      await lx.tabBar.update({
        items: [{ index: 1, badge: 'chrome' }],
      });
    `,
  });
  await waitForTabBar(
    (state) => state.items[1]?.badge === 'chrome',
    'home tabBar item patch after chrome refresh',
  );
  const viewportAfterChromeRefresh = await readHomeViewportHeight();
  expect(viewportAfterChromeRefresh).toBe(viewportBeforeChromeRefresh);
});

spec('rejects invalid native-surface dimensions before opening a host surface', {
  timeout: 60_000,
}, async (t) => {
  const app = t.apps.lxapp(SHOWCASE_APP_ID);
  await app.nav.relaunch({ page: 'ui', query: { type: 'surface' } });
  await app.page.waitFor({ page: 'ui', css: '[data-testid="open-surface"]' });
  await app.page.scrollTo({ page: 'ui', css: '[data-testid="open-surface"]' });

  await app.view.css('input[placeholder="width (px or %)"]', { page: 'ui' }).fill('invalid');
  await app.view.css('input[placeholder="height (px or %)"]', { page: 'ui' }).fill('50%');
  await waitForElementAttribute(app, 'ui', '[data-testid="open-surface"]', 'data-surface-width', 'invalid');
  await waitForElementAttribute(app, 'ui', '[data-testid="open-surface"]', 'data-surface-height', '50%');
  await app.view.testId("open-surface", { page: 'ui' }).click();
  await app.page.waitFor({ page: 'ui', css: '[data-testid="size-error"]' });

  const error = await app.page.query({ page: 'ui', css: '[data-testid="size-error"]', full: true });
  expect(error.exists).toBeTruthy();
  expect(error.exists && error.text.trim().length > 0).toBeTruthy();
});
