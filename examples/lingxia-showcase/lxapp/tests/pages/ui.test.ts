import { expect, spec, type LogicPage } from '@lingxia/test';
import type { LxAppRuntimeTabBarInfo } from '@lingxia/types/automation';
import { waitForElementAttribute, waitForCurrentPage } from '../helpers/page.js';
import { bindFixture, eventually, specNamespace, type Caught } from '../helpers/poll.js';
import { SHOWCASE_APP_ID } from '../helpers/app.js';

spec("run navigation APIs from the rendered UI controls", { id: "UI-NAV-001", covers: ['lx.navigateTo', 'lx.navigateBack', 'lx.redirectTo', 'lx.switchTab'], app: SHOWCASE_APP_ID }, async (t) => {
  const { app } = bindFixture(t, "UI-NAV-001");

  await app.nav.relaunch({ page: 'ui', query: { type: 'navigation' } });
  const ui = await app.page({ name: 'ui' }, { timeout: 30_000 });
  await ui.view.testId('ui-navigate-to').waitFor({ state: 'visible', timeout: 30_000 });

  await ui.view.testId('ui-navigate-to').click();
  await eventually(() => app.nav.stack(), (stack) => stack.length === 2, {
    describe: 'UI navigateTo to push a second page instance',
  });

  // The push created a fresh instance of this same route, now current (two
  // live `ui` instances, so bind it by position, not by name); wait for its
  // document before driving the next control.
  const pushed = await app.page();
  await pushed.view.testId('ui-navigate-back').waitFor({ state: 'visible', timeout: 30_000 });
  await pushed.view.testId('ui-navigate-back').click();
  await eventually(() => app.nav.stack(), (stack) => stack.length === 1, {
    describe: 'UI navigateBack to pop the page instance',
  });

  const readCurrentUiLifecycle = () => app.logic.eval(({ getCurrentPages }) => {
    const data = getCurrentPages<LogicPage<{ instanceTag?: string; events?: string[] }>>()
      .find((page) => page.route.includes('/ui/'))?.data;
    return {
      instanceTag: data?.instanceTag ?? '',
      onLoadCount: (data?.events ?? []).filter((event) => event.endsWith('onLoad')).length,
    };
  });
  await eventually(
    readCurrentUiLifecycle,
    ({ onLoadCount }) => onLoadCount > 0,
    { describe: 'current UI page onLoad count before redirect' },
  );
  await app.logic.eval(({ getCurrentPages }) => {
    const page = getCurrentPages<LogicPage<{ events?: string[] }>>()
      .find((candidate) => candidate.route.includes('/ui/'));
    if (!page) throw new Error('current UI PageInstance is missing');
    page.data.events = [];
  });
  // The pop returned to the first instance, which `ui` still holds.
  await ui.view.testId('ui-redirect-to').click();
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
  const replaced = await app.page({ name: 'ui' });
  await waitForElementAttribute(
    replaced.view,
    '[data-testid="ui-page"]',
    'data-instance-tag',
    redirected.instanceTag,
  );

  await replaced.view.testId('ui-switch-tab').click();
  await waitForCurrentPage(app, 'home');
  expect((await app.nav.stack()).map(({ name }) => name)).toEqual(['home']);
});

spec("apply TabBar visibility, style, item, icon, badge, and red-dot updates", { id: "UI-TABBAR-001", covers: ['lx.tabBar', 'lx.tabBar.update'], app: SHOWCASE_APP_ID, timeout: 60_000 }, async (t) => {
  const { app, defer } = bindFixture(t, "UI-TABBAR-001");

  const tabBar = async (): Promise<LxAppRuntimeTabBarInfo> => {
    const state = (await app.info()).tabBar;
    if (state === null) throw new Error('showcase TabBar is not declared');
    return state;
  };
  const waitForTabBar = (
    accept: (state: LxAppRuntimeTabBarInfo) => boolean,
    describe: string,
  ) => eventually(tabBar, accept, { describe });

  defer(async () => {
    await app.logic.eval(async ({ lx }) => {
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
    });
    await app.nav.relaunch({ page: 'home' });
  });

  await app.nav.relaunch({ page: 'ui', query: { type: 'tabbar' } });
  const ui = await app.page({ name: 'ui' }, { timeout: 30_000 });
  await ui.view.testId('tabbar-show').waitFor({ state: 'visible', timeout: 30_000 });

  const automaticDetail = await waitForTabBar(
    ({ visibility, routeVisible, effectiveVisible }) => (
      visibility === 'auto' && !routeVisible && !effectiveVisible
    ),
    'automatic TabBar visibility on a non-tab route',
  );
  expect(automaticDetail.selectedIndex).toBe(-1);

  await ui.view.testId('tabbar-show').click();
  const forced = await waitForTabBar(
    ({ visibility, routeVisible, effectiveVisible }) => (
      visibility === 'visible' && !routeVisible && effectiveVisible
    ),
    'forced TabBar visibility on a non-tab route',
  );
  expect(forced.effectiveVisible).toBeTruthy();

  await app.logic.eval(async ({ lx }) => {
    await lx.tabBar.update({
      items: [{
        index: 1,
        text: 'Automation',
        iconPath: 'public/home.png',
        badge: '7',
      }],
    });
  });
  // The runtime resolves relative icon paths against the package directory
  // with native separators, so Windows reports them with backslashes.
  const assetPath = (value: string | null | undefined) => (value ?? '').replace(/\\/g, '/');
  const styled = await waitForTabBar(
    (state) => (
      state.items[1]?.text === 'Automation'
      && assetPath(state.items[1]?.iconPath).endsWith('/public/home.png')
      && state.items[1]?.badge === '7'
      && state.items[1]?.redDot === false
    ),
    'TabBar text and badge update',
  );

  const invalid: Caught = await app.logic.eval(async ({ lx }) => {
    try {
      await lx.tabBar.update({ visibility: 'hidden', items: [{ index: 99, text: 'Invalid' }] });
        return { ok: true };
    } catch (error) {
      const { code, message, data } = error as { code?: string; message?: string; data?: unknown };
      return { ok: false, code: code ?? '', message: String(message ?? error), data: data ?? null };
    }
  });
  expect(invalid.ok).toBeFalsy();
  expect(invalid.code).toBe('E_INVALID_ARG');
  expect(await tabBar()).toEqual(styled);

  await app.logic.eval(async ({ lx }) => {
    await lx.tabBar.update({
      items: [{ index: 1, badge: null, redDot: true }],
    });
  });
  await waitForTabBar(
    (state) => state.items[1]?.badge === null && state.items[1]?.redDot === true,
    'TabBar badge replacement by a red dot',
  );

  await ui.view.testId('tabbar-hide').click();
  await waitForTabBar(
    ({ visibility, effectiveVisible }) => visibility === 'hidden' && !effectiveVisible,
    'explicitly hidden TabBar',
  );

  await app.logic.eval(async ({ lx }) => {
    await lx.tabBar.update({ visibility: 'auto' });
  });
  await waitForTabBar(
    ({ visibility, routeVisible, effectiveVisible }) => (
      visibility === 'auto' && !routeVisible && !effectiveVisible
    ),
    'restored automatic visibility on a non-tab route',
  );

  await app.nav.relaunch({ page: 'home' });
  const home = await app.page({ name: 'home' }, { timeout: 30_000 });
  await home.view.css('body').waitFor({ state: 'attached', timeout: 30_000 });
  await waitForTabBar(
    ({ visibility, routeVisible, effectiveVisible, selectedIndex }) => (
      visibility === 'auto' && routeVisible && effectiveVisible && selectedIndex === 0
    ),
    'automatic visibility after entering a tab route',
  );

  const readHomeViewportHeight = () => home.view.eval(({ window }) => window.innerHeight);
  const viewportBeforeChromeRefresh = await eventually(
    readHomeViewportHeight,
    (height) => height > 0,
    { describe: 'home WebView to expose a non-zero viewport' });
  await app.logic.eval(async ({ lx }) => {
    await lx.tabBar.update({
      items: [{ index: 1, badge: 'chrome' }],
    });
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
  const app = t.automation.lxapp(SHOWCASE_APP_ID);
  await app.nav.relaunch({ page: 'ui', query: { type: 'surface' } });
  const ui = await app.page({ name: 'ui' }, { timeout: 30_000 });
  await ui.view.testId('open-surface').waitFor({ timeout: 30_000 });

  await ui.view.css('input[placeholder="width (px or %)"]').fill('invalid');
  await ui.view.css('input[placeholder="height (px or %)"]').fill('50%');
  await waitForElementAttribute(ui.view, '[data-testid="open-surface"]', 'data-surface-width', 'invalid');
  await waitForElementAttribute(ui.view, '[data-testid="open-surface"]', 'data-surface-height', '50%');
  await ui.view.testId('open-surface').click();
  const sizeError = ui.view.testId('size-error');
  await sizeError.waitFor({ timeout: 30_000 });

  expect((await sizeError.textContent()).length).toBeGreaterThan(0);
});
