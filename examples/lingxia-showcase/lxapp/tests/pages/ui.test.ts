import { expect, test } from '@rongjs/test';
import type { LxAppRuntimeTabBarInfo } from 'lingxia-types/automation';
import { showcaseApp } from '../helpers/app.js';
import { waitForElementAttribute } from '../helpers/page.js';
import { waitForCurrentPage } from '../helpers/page.js';
import { contract, eventually } from '../support/contract.js';

contract({
  id: 'UI-NAV-001',
  title: 'run navigation APIs from the rendered UI controls',
  covers: ['lx.navigateTo', 'lx.navigateBack', 'lx.redirectTo', 'lx.switchTab'],
  layer: 'logic',
  levels: ['semantic', 'boundary', 'lifecycle'],
  scope: 'portable',
  expectedOutcome: 'supported',
}, async ({ app }) => {
  await app.nav.relaunch({ page: 'ui', query: { type: 'navigation' } });
  await app.page.waitFor({ page: 'ui', css: '[data-testid="ui-navigate-to"]', state: 'visible' });

  await app.page.click({ page: 'ui', css: '[data-testid="ui-navigate-to"]' });
  await eventually(() => app.nav.stack(), (stack) => stack.length === 2, {
    describe: 'UI navigateTo to push a second page instance',
  });

  // The push created a fresh instance of this same route; wait for its
  // document before driving the next control.
  await app.page.waitFor({ page: 'ui', css: '[data-testid="ui-navigate-back"]', state: 'visible' });
  await app.page.click({ page: 'ui', css: '[data-testid="ui-navigate-back"]' });
  await eventually(() => app.nav.stack(), (stack) => stack.length === 1, {
    describe: 'UI navigateBack to pop the page instance',
  });

  await app.page.click({ page: 'ui', css: '[data-testid="ui-redirect-to"]' });
  await eventually(() => app.nav.stack(), (stack) => stack.length === 1 && stack[0]?.name === 'ui', {
    describe: 'UI redirectTo to replace the current page',
  });

  await app.page.click({ page: 'ui', css: '[data-testid="ui-switch-tab"]' });
  await waitForCurrentPage(app, 'home');
  expect((await app.nav.stack()).map(({ name }) => name)).toEqual(['home']);
});

contract({
  id: 'UI-TABBAR-001',
  title: 'apply TabBar visibility, style, item, icon, badge, and red-dot updates',
  covers: ['lx.tabBar', 'lx.tabBar.update'],
  layer: 'logic',
  levels: ['semantic', 'boundary', 'lifecycle'],
  scope: 'portable',
  expectedOutcome: 'supported',
}, async ({ app, defer }) => {
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
          style: null,
          items: [{
            index: 1,
            text: null,
            iconPath: null,
            selectedIconPath: null,
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

  await app.page.click({ page: 'ui', css: '[data-testid="tabbar-show"]' });
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
        style: {
          foregroundColor: '#102030',
          selectedForegroundColor: '#405060',
        },
        items: [{
          index: 1,
          text: 'Automation',
          iconPath: 'public/home.png',
          selectedIconPath: 'public/home_selected.png',
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
      state.runtime_style.foreground_color === '#102030'
      && state.runtime_style.selected_foreground_color === '#405060'
      && state.items[1]?.text === 'Automation'
      && assetPath(state.items[1]?.icon_path).endsWith('/public/home.png')
      && assetPath(state.items[1]?.selected_icon_path).endsWith('/public/home_selected.png')
      && state.items[1]?.badge === '7'
      && state.items[1]?.red_dot === false
    ),
    'TabBar style, text, and badge update',
  );

  let invalidRejected = false;
  try {
    await app.eval({
      script: `
        await lx.tabBar.update({
          visibility: 'hidden',
          items: [{ index: 99, text: 'Invalid' }],
        });
      `,
    });
  } catch {
    invalidRejected = true;
  }
  expect(invalidRejected).toBeTruthy();
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

  await app.page.click({ page: 'ui', css: '[data-testid="tabbar-hide"]' });
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
    { describe: 'home WebView to expose a non-zero viewport' },
  );
  await app.eval({
    script: `
      await lx.tabBar.update({
        style: {
          foregroundColor: '#203040',
          selectedForegroundColor: '#506070',
        },
      });
    `,
  });
  // Android refreshes the complete native page-chrome revision. A WebView
  // source path such as index.tsx must still resolve to this custom-navbar
  // page instead of falling back to a visible default NavigationBar.
  await new Promise<void>((resolve) => setTimeout(resolve, 500));
  const viewportAfterChromeRefresh = await readHomeViewportHeight();
  expect(viewportAfterChromeRefresh).toBe(viewportBeforeChromeRefresh);
});

test('rejects invalid native-surface dimensions before opening a host surface', async () => {
  const app = showcaseApp();
  await app.nav.relaunch({ page: 'ui', query: { type: 'surface' } });
  await app.page.waitFor({ page: 'ui', css: '[data-testid="open-surface"]' });

  await app.page.fill({ page: 'ui', css: 'input[placeholder="width (px or %)"]', text: 'invalid' });
  await app.page.fill({ page: 'ui', css: 'input[placeholder="height (px or %)"]', text: '50%' });
  await waitForElementAttribute(app, 'ui', '[data-testid="open-surface"]', 'data-surface-width', 'invalid');
  await waitForElementAttribute(app, 'ui', '[data-testid="open-surface"]', 'data-surface-height', '50%');
  await app.page.click({ page: 'ui', css: '[data-testid="open-surface"]' });
  await app.page.waitFor({ page: 'ui', css: '[data-testid="size-error"]' });

  const error = await app.page.query({ page: 'ui', css: '[data-testid="size-error"]', full: true });
  expect(error.exists).toBeTruthy();
  expect(error.exists && error.text.trim().length > 0).toBeTruthy();
});
