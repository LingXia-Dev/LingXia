import { expect, spec } from '@lingxia/test';
import type { LxAppDriver, LxAppRuntimeNavigationBarInfo, LxAppRuntimeTabBarInfo } from 'lingxia-types/automation';
import { bindFixture, evalCaught, eventually, relaunchFromLogic } from '../helpers/poll.js';
import { SHOWCASE_APP_ID } from '../helpers/app.js';
import { waitForCurrentPage } from '../helpers/page.js';
import { runtimePlatform } from '../helpers/platform.js';

function hex(value: string | null | undefined): string {
  return (value ?? '').replace(/\\/g, '/').toUpperCase();
}

async function navigationBar(app: LxAppDriver): Promise<LxAppRuntimeNavigationBarInfo> {
  const state = (await app.info()).navigation_bar;
  if (state === null || state === undefined) throw new Error('showcase NavigationBar snapshot is missing');
  return state;
}

async function waitForNavBar(
  app: LxAppDriver,
  accept: (state: LxAppRuntimeNavigationBarInfo) => boolean,
  describe: string,
): Promise<LxAppRuntimeNavigationBarInfo> {
  return eventually(() => navigationBar(app), accept, { describe });
}

async function tabBar(app: LxAppDriver): Promise<LxAppRuntimeTabBarInfo> {
  const state = (await app.info()).tab_bar;
  if (state === null) throw new Error('showcase TabBar is not declared');
  return state;
}

async function waitForTabBar(
  app: LxAppDriver,
  accept: (state: LxAppRuntimeTabBarInfo) => boolean,
  describe: string,
): Promise<LxAppRuntimeTabBarInfo> {
  return eventually(() => tabBar(app), accept, { describe });
}

async function appearanceOf(app: LxAppDriver): Promise<{ preference: string; resolved: string }> {
  return app.eval({
    script: 'return lx.appearance.get();',
  }) as Promise<{ preference: string; resolved: string }>;
}

spec("apply navigationBar title, colors, home button, and reset", {
  id: "UI-NAVBAR-001",
  covers: ['lx.navigationBar', 'lx.navigationBar.update'],
  app: SHOWCASE_APP_ID,
  timeout: 60_000,
}, async (t) => {
  const { app, defer } = bindFixture(t, "UI-NAVBAR-001");
  defer(async () => {
    await app.eval({
      script: `await lx.navigationBar.update({ title: null, style: null, homeButton: 'auto' });`,
    }).catch(() => undefined);
  });

  await t.step('drive the ui page presets', async () => {
    await app.nav.relaunch({ page: 'ui', query: { type: 'navbar' } });
    await app.page.waitFor({ page: 'ui', css: '[data-testid="navbar-preset-blue"]', state: 'visible' });
    await app.page.click({ page: 'ui', css: '[data-testid="navbar-preset-blue"]' });
    const styled = await waitForNavBar(
      app,
      (state) => state.title === 'Blue Theme'
        && hex(state.runtime_style.background_color) === '#3B82F6'
        && hex(state.runtime_style.foreground_color) === '#FFFFFF',
      'blue navigationBar preset',
    );
    expect(styled.home_button).toBe('auto');
  });

  await t.step('set divider color and hide the home button', async () => {
    await app.eval({
      script: `
        await lx.navigationBar.update({
          style: { dividerColor: '#112233' },
          homeButton: 'hidden',
        });
      `,
    });
    const patched = await waitForNavBar(
      app,
      (state) => hex(state.runtime_style.divider_color) === '#112233'
        && state.home_button === 'hidden'
        && state.home_button_visible === false,
      'divider color and hidden home button',
    );
    expect(hex(patched.runtime_style.background_color)).toBe('#3B82F6');
  });

  await t.step('restore the home button from the page control', async () => {
    await app.page.click({ page: 'ui', css: '[data-testid="navbar-home-auto"]' });
    await waitForNavBar(app, (state) => state.home_button === 'auto', 'auto home button');
  });

  await t.step('reject an invalid color without mutating chrome', async () => {
    const before = await navigationBar(app);
    const rejected = await evalCaught(
      app,
      `await lx.navigationBar.update({ style: { backgroundColor: 'not-a-color' } });`,
    );
    expect(rejected.ok).toBeFalsy();
    expect(rejected.code).toBe('E_INVALID_ARG');
    expect(await navigationBar(app)).toEqual(before);
  });

  await t.step('reset title and style with null', async () => {
    await app.page.click({ page: 'ui', css: '[data-testid="navbar-reset"]' });
    await waitForNavBar(
      app,
      (state) => state.title === 'User Interface'
        && state.runtime_style.background_color === null
        && state.runtime_style.foreground_color === null
        && state.runtime_style.divider_color === null
        && state.home_button === 'auto',
      'navigationBar reset to manifest title',
    );
  });

  await t.step('survive reLaunch', async () => {
    await app.eval({
      script: `
        await lx.navigationBar.update({
          title: 'Kept Title',
          style: { backgroundColor: '#10B981', foregroundColor: '#FFFFFF', dividerColor: '#0F766E' },
        });
      `,
    });
    await waitForNavBar(app, (state) => state.title === 'Kept Title', 'title before relaunch');
    await app.nav.relaunch({ page: 'ui', query: { type: 'navbar' } });
    await waitForCurrentPage(app, 'ui', 30_000);
    await app.page.waitFor({ page: 'ui', css: '[data-testid="navbar-preset-blue"]', state: 'visible' });
    // Page onLoad sets the demo title again; the style patch is page-scoped and
    // a new instance starts from the manifest unless Logic reapplies it.
    const after = await navigationBar(app);
    expect(after.title).toBe('Navigation Bar Demo');
  });
});

spec("round-trip appearance preference through the ui controls", {
  id: "UI-APPEARANCE-001",
  covers: ['lx.appearance', 'lx.appearance.get', 'lx.appearance.set'],
  app: SHOWCASE_APP_ID,
  timeout: 60_000,
}, async (t) => {
  const { app, defer } = bindFixture(t, "UI-APPEARANCE-001");
  defer(async () => {
    await app.eval({ script: `await lx.appearance.set('auto');` }).catch(() => undefined);
  });

  await app.nav.relaunch({ page: 'ui', query: { type: 'appearance' } });
  await app.page.waitFor({ page: 'ui', css: '[data-testid="ui-appearance-light"]', state: 'visible' });

  for (const preference of ['light', 'dark', 'auto'] as const) {
    await t.step(`set ${preference}`, async () => {
      await app.page.click({ page: 'ui', css: `[data-testid="ui-appearance-${preference}"]` });
      await eventually(
        () => appearanceOf(app),
        (state) => state.preference === preference,
        { describe: `appearance preference ${preference}` },
      );
      const state = await appearanceOf(app);
      if (preference === 'auto') {
        expect(['light', 'dark']).toContain(state.resolved);
      } else {
        expect(state.resolved).toBe(preference);
      }
      await eventually(
        () => app.page.eval({
          page: 'ui',
          script: `document.querySelector('[data-testid="ui-appearance-preference"]')?.textContent ?? ''`,
        }) as Promise<string>,
        (text) => text === state.preference,
        { describe: `ui appearance label ${preference}`, timeoutMs: 5_000 },
      );
    });
  }

  await t.step('reject an invalid preference and keep the previous state', async () => {
    const before = await appearanceOf(app);
    const rejected = await evalCaught(app, `await lx.appearance.set('sepia');`);
    expect(rejected.ok).toBeFalsy();
    expect(rejected.code).toBe('E_INVALID_ARG');
    expect(await appearanceOf(app)).toEqual(before);
  });

  await t.step('persist across relaunch', async () => {
    await app.page.click({ page: 'ui', css: '[data-testid="ui-appearance-dark"]' });
    await eventually(() => appearanceOf(app), (state) => state.preference === 'dark', {
      describe: 'dark preference before relaunch',
    });
    await relaunchFromLogic(app, 'ui', { type: 'appearance' });
    await waitForCurrentPage(app, 'ui', 30_000);
    await app.page.waitFor({ page: 'ui', css: '[data-testid="ui-appearance-dark"]', state: 'visible' });
    const after = await appearanceOf(app);
    expect(after.preference).toBe('dark');
    expect(after.resolved).toBe('dark');
  });
});

/**
 * Where the feedback overlays live differs by host, so the scenario is one and
 * the difference is a profile: a desktop draws them in the page from
 * `@lingxia/bridge`, a phone draws native ones the page cannot see and only a
 * system tap can answer.
 */
const OVERLAY_SURFACE: Record<string, 'dom' | 'native'> = {
  macos: 'dom',
  windows: 'dom',
  android: 'native',
  ios: 'native',
  harmony: 'native',
};

spec("show, hide, confirm, and cancel in-app feedback overlays", {
  id: "UI-FEEDBACK-001",
  covers: ['lx.showToast', 'lx.hideToast', 'lx.showModal', 'lx.showActionSheet'],
  app: SHOWCASE_APP_ID,
  reason: 'Mobile hosts render feedback through native overlays rather than DOM elements.',
  timeout: 60_000,
}, async (t) => {
  const { app } = bindFixture(t, "UI-FEEDBACK-001");
  const platform = await runtimePlatform(app);
  const overlays = OVERLAY_SURFACE[platform] ?? 'dom';

  await t.step('show and hide a toast', async () => {
    await app.nav.relaunch({ page: 'ui', query: { type: 'toast' } });
    await app.page.waitFor({ page: 'ui', css: '[data-testid="toast-show"]', state: 'visible' });
    await app.eval({
      script: `await lx.showToast({ title: 'Coverage toast', icon: 'none', duration: 8000 });`,
    });
    if (overlays === 'dom') {
      await app.page.waitFor({ page: 'ui', css: '.lx-toast-title', state: 'visible' });
      const shown = await app.page.eval({
        page: 'ui',
        script: `document.querySelector('.lx-toast-title')?.textContent ?? ''`,
      });
      expect(shown).toBe('Coverage toast');
    } else {
      // A native toast is not in the page; what a caller can rely on is that
      // showing it does not put anything in the page either.
      const leaked = await app.page.eval({
        page: 'ui',
        script: `document.querySelector('.lx-toast-title') ? 'yes' : 'no'`,
      });
      expect(leaked).toBe('no');
    }
    await app.eval({ script: `await lx.hideToast();` });
    if (overlays === 'dom') {
      await eventually(
        () => app.page.eval({
          page: 'ui',
          script: `document.querySelector('.lx-toast-title') ? 'yes' : 'no'`,
        }),
        (value) => value === 'no',
        { describe: 'toast to disappear after hideToast' },
      );
    }
  });

  // A native modal or action sheet only answers a system tap, and this suite
  // has no driver for one; on those hosts the dialogs stay unproven rather
  // than leaving a pending promise and a dialog on screen.
  if (overlays !== 'dom') return;

  await t.step('confirm and cancel a modal', async () => {
    const confirmed = app.eval({
      script: `return await lx.showModal({ title: 'Coverage', content: 'Confirm this', showCancel: true, confirmText: 'OK', cancelText: 'Cancel' });`,
    }) as Promise<{ canceled: boolean }>;
    await app.page.waitFor({ page: 'ui', css: '.lx-modal-btn-confirm', state: 'visible' });
    await app.page.click({ page: 'ui', css: '.lx-modal-btn-confirm' });
    expect((await confirmed).canceled).toBeFalsy();

    const canceled = app.eval({
      script: `return await lx.showModal({ title: 'Coverage', content: 'Cancel this', showCancel: true });`,
    }) as Promise<{ canceled: boolean }>;
    await app.page.waitFor({ page: 'ui', css: '.lx-modal-btn-cancel', state: 'visible' });
    await app.page.click({ page: 'ui', css: '.lx-modal-btn-cancel' });
    expect((await canceled).canceled).toBeTruthy();

    const noCancel = app.eval({
      script: `return await lx.showModal({ content: 'No cancel', showCancel: false });`,
    }) as Promise<{ canceled: boolean }>;
    await app.page.waitFor({ page: 'ui', css: '.lx-modal-btn-confirm', state: 'visible' });
    const cancelCount = await app.page.eval({
      page: 'ui',
      script: `document.querySelectorAll('.lx-modal-btn-cancel').length`,
    });
    expect(cancelCount).toBe(0);
    await app.page.click({ page: 'ui', css: '.lx-modal-btn-confirm' });
    expect((await noCancel).canceled).toBeFalsy();
  });

  await t.step('pick and dismiss an action sheet', async () => {
    const picked = app.eval({
      script: `return await lx.showActionSheet({ itemList: ['View Details', '查看日志', 'Send Email', '删除'] });`,
    }) as Promise<{ canceled: boolean; index?: number }>;
    await app.page.waitFor({ page: 'ui', css: '.lx-as-item', state: 'visible' });
    await app.page.click({ page: 'ui', css: '.lx-as-item', index: 1 });
    const selected = await picked;
    expect(selected.canceled).toBeFalsy();
    expect(selected.index).toBe(1);

    const dismissed = app.eval({
      script: `return await lx.showActionSheet({ itemList: ['One', 'Two'] });`,
    }) as Promise<{ canceled: boolean }>;
    await app.page.waitFor({ page: 'ui', css: '.lx-as-cancel-btn', state: 'visible' });
    await app.page.click({ page: 'ui', css: '.lx-as-cancel-btn' });
    expect((await dismissed).canceled).toBeTruthy();
  });

  await t.step('reject an empty action sheet', async () => {
    const rejected = await evalCaught(app, `await lx.showActionSheet({ itemList: [] });`);
    expect(rejected.ok).toBeFalsy();
    expect(rejected.code).toBe('E_INVALID_ARG');
  });
});

spec("assert tabBar failure codes, resets, and button-driven patches", {
  id: "UI-TABBAR-002",
  covers: ['lx.tabBar', 'lx.tabBar.update', 'lx.switchTab'],
  app: SHOWCASE_APP_ID,
  timeout: 60_000,
}, async (t) => {
  const { app, defer } = bindFixture(t, "UI-TABBAR-002");
  defer(async () => {
    await app.eval({
      script: `
        await lx.tabBar.update({
          visibility: 'auto',
          style: null,
          items: [{
            index: 0,
            text: null,
            iconPath: null,
            badge: null,
            redDot: false,
          }, {
            index: 1,
            text: null,
            iconPath: null,
            badge: null,
            redDot: false,
          }],
        });
      `,
    }).catch(() => undefined);
    await app.nav.relaunch({ page: 'home' }).catch(() => undefined);
  });

  await app.nav.relaunch({ page: 'ui', query: { type: 'tabbar' } });
  await app.page.waitFor({ page: 'ui', css: '[data-testid="tabbar-show"]', state: 'visible' });
  await app.page.click({ page: 'ui', css: '[data-testid="tabbar-show"]' });
  await waitForTabBar(app, (state) => state.effective_visible, 'forced tab bar');

  await t.step('drive showcase badge and red-dot buttons', async () => {
    await app.page.click({ page: 'ui', css: '[data-testid="tabbar-reddot-show"]' });
    await waitForTabBar(app, (state) => state.items[1]?.red_dot === true, 'red dot from button');
    await app.page.fill({ page: 'ui', css: '[data-testid="tabbar-badge-input"]', text: '9' });
    await app.page.click({ page: 'ui', css: '[data-testid="tabbar-badge-set"]' });
    await waitForTabBar(app, (state) => state.items[1]?.badge === '9', 'badge from button');
    await app.page.fill({ page: 'ui', css: '[data-testid="tabbar-item-text"]', text: 'API+' });
    await app.page.click({ page: 'ui', css: '[data-testid="tabbar-item-update"]' });
    await waitForTabBar(app, (state) => state.items[1]?.text === 'API+', 'item text from button');
  });

  await t.step('patch the selected tab and a second item together', async () => {
    await app.eval({
      script: `
        await lx.tabBar.update({
          items: [
            { index: 0, badge: '1' },
            { index: 1, badge: '2', redDot: false },
          ],
        });
      `,
    });
    await waitForTabBar(
      app,
      (state) => state.items[0]?.badge === '1' && state.items[1]?.badge === '2',
      'multi-item tabBar patch',
    );
  });

  await t.step('reset style with null', async () => {
    await app.eval({
      script: `
        await lx.tabBar.update({
          style: { foregroundColor: '#102030', selectedForegroundColor: '#405060' },
        });
      `,
    });
    await waitForTabBar(
      app,
      (state) => state.runtime_style.foreground_color === '#102030',
      'custom tabBar style',
    );
    await app.eval({ script: `await lx.tabBar.update({ style: null });` });
    await waitForTabBar(
      app,
      (state) => state.runtime_style.foreground_color === null
        && state.runtime_style.selected_foreground_color === null,
      'tabBar style reset',
    );
  });

  await t.step('reject invalid updates with E_INVALID_ARG', async () => {
    const before = await tabBar(app);
    for (const body of [
      `await lx.tabBar.update({ items: [{ index: 99, text: 'Invalid' }] });`,
      `await lx.tabBar.update({ items: [{ index: -1, text: 'Invalid' }] });`,
      `await lx.tabBar.update({ style: { foregroundColor: 'not-a-color' } });`,
      `await lx.tabBar.update({ items: [{ index: 1, iconPath: '../secret.png' }] });`,
    ]) {
      const rejected = await evalCaught(app, body);
      expect(rejected.ok).toBeFalsy();
      expect(rejected.code).toBe('E_INVALID_ARG');
      expect(await tabBar(app)).toEqual(before);
    }
  });

  await t.step('last concurrent update wins without a torn item', async () => {
    await app.eval({
      script: `
        await Promise.all([
          lx.tabBar.update({ items: [{ index: 1, text: 'First', badge: 'A' }] }),
          lx.tabBar.update({ items: [{ index: 1, text: 'Second', badge: 'B' }] }),
        ]);
      `,
    });
    const after = await waitForTabBar(
      app,
      (state) => state.items[1]?.text === 'First' || state.items[1]?.text === 'Second',
      'one of the concurrent tabBar writes',
    );
    const item = after.items[1];
    const consistent = (item?.text === 'First' && item?.badge === 'A')
      || (item?.text === 'Second' && item?.badge === 'B');
    expect(consistent).toBeTruthy();
  });

  await t.step('switchTab agrees with selected_index after a badge change', async () => {
    await app.eval({ script: `void lx.switchTab({ page: 'todo' }); return 'scheduled';` });
    await waitForCurrentPage(app, 'todo');
    const state = await waitForTabBar(
      app,
      (item) => item.selected_index === 3 && item.route_visible,
      'todo tab selected',
    );
    expect(state.items[1]?.text === 'API+' || state.items[1]?.text === 'Second' || state.items[1]?.text === 'First')
      .toBeTruthy();
  });
});
