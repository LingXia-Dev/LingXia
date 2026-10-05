import type { JsonValue, TestApp } from '@lingxia/test';
import { expect, spec } from '@lingxia/test';
import type { LxAppRuntimeNavigationBarInfo, LxAppRuntimeTabBarInfo } from '@lingxia/types/automation';
import { bindFixture, eventually, relaunchFromLogic, type Caught } from '../helpers/poll.js';
import { SHOWCASE_APP_ID } from '../helpers/app.js';
import { waitForCurrentPage } from '../helpers/page.js';
import { runtimePlatform } from '../helpers/platform.js';

function hex(value: string | null | undefined): string {
  return (value ?? '').replace(/\\/g, '/').toUpperCase();
}

async function navigationBar(app: TestApp): Promise<LxAppRuntimeNavigationBarInfo> {
  const state = (await app.info()).navigationBar;
  if (state === null || state === undefined) throw new Error('showcase NavigationBar snapshot is missing');
  return state;
}

async function waitForNavBar(
  app: TestApp,
  accept: (state: LxAppRuntimeNavigationBarInfo) => boolean,
  describe: string,
): Promise<LxAppRuntimeNavigationBarInfo> {
  return eventually(() => navigationBar(app), accept, { describe });
}

async function tabBar(app: TestApp): Promise<LxAppRuntimeTabBarInfo> {
  const state = (await app.info()).tabBar;
  if (state === null) throw new Error('showcase TabBar is not declared');
  return state;
}

async function waitForTabBar(
  app: TestApp,
  accept: (state: LxAppRuntimeTabBarInfo) => boolean,
  describe: string,
): Promise<LxAppRuntimeTabBarInfo> {
  return eventually(() => tabBar(app), accept, { describe });
}

async function appearanceOf(app: TestApp): Promise<{ preference: string; resolved: string }> {
  return app.logic.eval(({ lx }) => ({
    preference: String(lx.host.control?.appearance.getPreference()),
    resolved: String(lx.host.appearance.get()),
  }));
}

/**
 * Apply a chrome patch the typings may refuse — the contract under test is
 * how the host answers it — and settle its rejection instead of throwing.
 */
async function caughtChromeUpdate(
  app: TestApp,
  target: 'navigationBar' | 'tabBar',
  patch: JsonValue,
): Promise<Caught> {
  return app.logic.eval(async ({ lx }, target, patch) => {
    try {
      await (lx[target].update as (patch: unknown) => Promise<void>)(patch);
      return { ok: true };
    } catch (error) {
      const { code, message, data } = error as { code?: string; message?: string; data?: unknown };
      return { ok: false, code: code ?? '', message: String(message ?? error), data: data ?? null };
    }
  }, target, patch);
}

spec("apply navigationBar title, colors, home button, and reset", {
  id: "UI-NAVBAR-001",
  covers: ['lx.navigationBar', 'lx.navigationBar.update'],
  app: SHOWCASE_APP_ID,
  timeout: 60_000,
}, async (t) => {
  const { app, defer } = bindFixture(t, "UI-NAVBAR-001");
  defer(async () => {
    await app.logic.eval(async ({ lx }) => {
      await lx.navigationBar.update({ title: null, style: null, homeButton: 'auto' });
    }).catch(() => undefined);
  });

  const ui = await t.step('drive the ui page presets', async () => {
    await app.nav.relaunch({ page: 'ui', query: { type: 'navbar' } });
    const page = await app.page({ name: 'ui' }, { timeout: 30_000 });
    await page.view.testId('navbar-preset-blue').click({ timeout: 30_000 });
    const styled = await waitForNavBar(
      app,
      (state) => state.title === 'Blue Theme'
        && hex(state.runtimeStyle.backgroundColor) === '#3B82F6'
        && hex(state.runtimeStyle.foregroundColor) === '#FFFFFF',
      'blue navigationBar preset',
    );
    expect(styled.homeButton).toBe('auto');
    return page;
  });

  await t.step('set divider color and hide the home button', async () => {
    await app.logic.eval(async ({ lx }) => {
      await lx.navigationBar.update({
        style: { dividerColor: '#112233' },
        homeButton: 'hidden',
      });
    });
    const patched = await waitForNavBar(
      app,
      (state) => hex(state.runtimeStyle.dividerColor) === '#112233'
        && state.homeButton === 'hidden'
        && state.homeButtonVisible === false,
      'divider color and hidden home button',
    );
    expect(hex(patched.runtimeStyle.backgroundColor)).toBe('#3B82F6');
  });

  await t.step('restore the home button from the page control', async () => {
    await ui.view.testId('navbar-home-auto').click();
    await waitForNavBar(app, (state) => state.homeButton === 'auto', 'auto home button');
  });

  await t.step('reject an invalid color without mutating chrome', async () => {
    const before = await navigationBar(app);
    const rejected = await caughtChromeUpdate(app, 'navigationBar', { style: { backgroundColor: 'not-a-color' } });
    expect(rejected.ok).toBeFalsy();
    expect(rejected.code).toBe('E_INVALID_ARG');
    expect(await navigationBar(app)).toEqual(before);
  });

  await t.step('reset title and style with null', async () => {
    await ui.view.testId('navbar-reset').click();
    await waitForNavBar(
      app,
      // The manifest title is declared per language; either is the reset.
      (state) => (state.title === 'User Interface' || state.title === '界面')
        && state.runtimeStyle.backgroundColor === null
        && state.runtimeStyle.foregroundColor === null
        && state.runtimeStyle.dividerColor === null
        && state.homeButton === 'auto',
      'navigationBar reset to manifest title',
    );
  });

  await t.step('survive reLaunch', async () => {
    await app.logic.eval(async ({ lx }) => {
      await lx.navigationBar.update({
        title: 'Kept Title',
        style: { backgroundColor: '#10B981', foregroundColor: '#FFFFFF', dividerColor: '#0F766E' },
      });
    });
    await waitForNavBar(app, (state) => state.title === 'Kept Title', 'title before relaunch');
    await app.nav.relaunch({ page: 'ui', query: { type: 'navbar' } });
    await waitForCurrentPage(app, 'ui', 30_000);
    const relaunched = await app.page({ name: 'ui' });
    await relaunched.view.testId('navbar-preset-blue').waitFor({ state: 'visible', timeout: 30_000 });
    // Page onLoad sets the demo title again; the style patch is page-scoped and
    // a new instance starts from the manifest unless Logic reapplies it.
    const after = await navigationBar(app);
    expect(after.title).toBe('Navigation Bar Demo');
  });
});

spec("round-trip appearance preference through the ui controls", {
  id: "UI-APPEARANCE-001",
  covers: [
    'lx.host.control',
    'lx.host.appearance.get',
    'lx.host.appearance.watch',
    'lx.host.control.appearance.getPreference',
    'lx.host.control.appearance.setPreference',
  ],
  app: SHOWCASE_APP_ID,
  timeout: 60_000,
}, async (t) => {
  const { app, defer } = bindFixture(t, "UI-APPEARANCE-001");
  defer(async () => {
    await app.logic.eval(async ({ lx }) => { await lx.host.control?.appearance.setPreference('auto'); })
      .catch(() => undefined);
  });

  await app.nav.relaunch({ page: 'ui', query: { type: 'appearance' } });
  const ui = await app.page({ name: 'ui' }, { timeout: 30_000 });
  await ui.view.testId('ui-appearance-light').waitFor({ state: 'visible', timeout: 30_000 });

  for (const preference of ['light', 'dark', 'auto'] as const) {
    await t.step(`set ${preference}`, async () => {
      await ui.view.testId(`ui-appearance-${preference}`).click();
      const state = await eventually(
        () => appearanceOf(app),
        (state) => state.preference === preference && (
          preference === 'auto'
            ? ['light', 'dark'].includes(state.resolved)
            : state.resolved === preference
        ),
        { describe: `appearance preference ${preference}` },
      );
      await eventually(
        () => ui.view.eval(({ document }) =>
          document.querySelector('[data-testid="ui-appearance-preference"]')?.textContent ?? ''),
        (text) => text === state.preference,
        { describe: `ui appearance label ${preference}`, timeoutMs: 5_000 },
      );
    });
  }

  await t.step('reject an invalid preference and keep the previous state', async () => {
    const before = await appearanceOf(app);
    const rejected: Caught = await app.logic.eval(async ({ lx }, preference) => {
      try {
        const control = lx.host.control!.appearance;
        await (control.setPreference as (preference: unknown) => Promise<void>)(preference);
        return { ok: true };
      } catch (error) {
        const { code, message, data } = error as { code?: string; message?: string; data?: unknown };
        return { ok: false, code: code ?? '', message: String(message ?? error), data: data ?? null };
      }
    }, 'sepia');
    expect(rejected.ok).toBeFalsy();
    expect(rejected.code).toBe('E_INVALID_ARG');
    expect(await appearanceOf(app)).toEqual(before);
  });

  await t.step('persist across relaunch', async () => {
    await ui.view.testId('ui-appearance-dark').click();
    await eventually(() => appearanceOf(app), (state) => state.preference === 'dark', {
      describe: 'dark preference before relaunch',
    });
    await relaunchFromLogic(app, 'ui', { type: 'appearance' });
    await waitForCurrentPage(app, 'ui', 30_000);
    const relaunched = await app.page({ name: 'ui' });
    await relaunched.view.testId('ui-appearance-dark').waitFor({ state: 'visible', timeout: 30_000 });
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
  covers: ['lx.showToast', 'lx.hideToast', 'lx.showModal', 'lx.alert', 'lx.confirm', 'lx.showActionSheet'],
  app: SHOWCASE_APP_ID,
  reason: 'Mobile hosts render feedback through native overlays rather than DOM elements.',
  timeout: 60_000,
}, async (t) => {
  const { app } = bindFixture(t, "UI-FEEDBACK-001");
  const platform = await runtimePlatform(app);
  const overlays = OVERLAY_SURFACE[platform] ?? 'dom';

  // Desktop overlays are drawn in the ui page's own document.
  const ui = await t.step('show and hide a toast', async () => {
    await app.nav.relaunch({ page: 'ui', query: { type: 'toast' } });
    const page = await app.page({ name: 'ui' }, { timeout: 30_000 });
    await page.view.testId('toast-show').waitFor({ state: 'visible', timeout: 30_000 });
    await app.logic.eval(async ({ lx }) => {
      await lx.showToast({ title: 'Coverage toast', icon: 'none', durationMs: 8000 });
    });
    if (overlays === 'dom') {
      await page.view.css('.lx-toast-title').first().waitFor({ state: 'visible', timeout: 30_000 });
      const shown = await page.view.eval(({ document }) =>
        document.querySelector('.lx-toast-title')?.textContent ?? '');
      expect(shown).toBe('Coverage toast');
    } else {
      // A native toast is not in the page; what a caller can rely on is that
      // showing it does not put anything in the page either.
      const leaked = await page.view.eval(({ document }) =>
        document.querySelector('.lx-toast-title') ? 'yes' : 'no');
      expect(leaked).toBe('no');
    }
    await app.logic.eval(async ({ lx }) => { await lx.hideToast(); });
    if (overlays === 'dom') {
      await eventually(
        () => page.view.eval(({ document }) =>
          document.querySelector('.lx-toast-title') ? 'yes' : 'no'),
        (value) => value === 'no',
        { describe: 'toast to disappear after hideToast' },
      );
    }
    return page;
  });

  // A native modal or action sheet only answers a system tap, and this suite
  // has no driver for one; on those hosts the dialogs stay unproven rather
  // than leaving a pending promise and a dialog on screen.
  if (overlays !== 'dom') return;

  await t.step('confirm and cancel a modal', async () => {
    const confirmed = app.logic.eval(({ lx }) =>
      lx.showModal({ title: 'Coverage', content: 'Confirm this', showCancel: true, confirmText: 'OK', cancelText: 'Cancel' }));
    await ui.view.css('.lx-modal-btn-confirm').first().waitFor({ state: 'visible', timeout: 30_000 });
    await ui.view.css('.lx-modal-btn-confirm').click();
    expect((await confirmed).status === 'canceled').toBeFalsy();

    const canceled = app.logic.eval(({ lx }) =>
      lx.showModal({ title: 'Coverage', content: 'Cancel this', showCancel: true }));
    await ui.view.css('.lx-modal-btn-cancel').first().waitFor({ state: 'visible', timeout: 30_000 });
    await ui.view.css('.lx-modal-btn-cancel').click();
    expect((await canceled).status === 'canceled').toBeTruthy();

    const noCancel = app.logic.eval(({ lx }) => lx.showModal({ content: 'No cancel', showCancel: false }));
    await ui.view.css('.lx-modal-btn-confirm').first().waitFor({ state: 'visible', timeout: 30_000 });
    const cancelCount = await ui.view.eval(({ document }) =>
      document.querySelectorAll('.lx-modal-btn-cancel').length);
    expect(cancelCount).toBe(0);
    await ui.view.css('.lx-modal-btn-confirm').click();
    expect((await noCancel).status === 'canceled').toBeFalsy();
  });

  await t.step('use acknowledgement and boolean dialogs', async () => {
    const alert = app.logic.eval(async ({ lx }) => {
      await lx.alert({ title: 'Notice' });
      return 'acknowledged';
    });
    await ui.view.css('.lx-modal-btn-confirm').first().waitFor({ state: 'visible', timeout: 30_000 });
    await ui.view.css('.lx-modal-btn-confirm').click();
    expect(await alert).toBe('acknowledged');
    const confirmed = app.logic.eval(({ lx }) => lx.confirm({ title: 'Continue?' }));
    await ui.view.css('.lx-modal-btn-confirm').first().waitFor({ state: 'visible', timeout: 30_000 });
    await ui.view.css('.lx-modal-btn-confirm').click();
    expect(await confirmed).toBe(true);
    const canceled = app.logic.eval(({ lx }) => lx.confirm({ title: 'Continue?' }));
    await ui.view.css('.lx-modal-btn-cancel').first().waitFor({ state: 'visible', timeout: 30_000 });
    await ui.view.css('.lx-modal-btn-cancel').click();
    expect(await canceled).toBe(false);
  });

  await t.step('pick and dismiss an action sheet', async () => {
    const picked = app.logic.eval(({ lx }) =>
      lx.showActionSheet({ items: ['View Details', '查看日志', 'Send Email', '删除'].map((label) => ({ id: label, label })) }));
    await ui.view.css('.lx-as-item').first().waitFor({ state: 'visible', timeout: 30_000 });
    await ui.view.css('.lx-as-item').nth(1).click();
    const selected = await picked;
    expect(selected.status === 'canceled').toBeFalsy();
    expect(selected.status === 'ok' ? selected.id : null).toBe('查看日志');

    const dismissed = app.logic.eval(({ lx }) =>
      lx.showActionSheet({ items: ['One', 'Two'].map((label) => ({ id: label, label })) }));
    await ui.view.css('.lx-as-cancel-btn').first().waitFor({ state: 'visible', timeout: 30_000 });
    await ui.view.css('.lx-as-cancel-btn').click();
    expect((await dismissed).status === 'canceled').toBeTruthy();
  });

  await t.step('reject an empty action sheet', async () => {
    const rejected: Caught = await app.logic.eval(async ({ lx }) => {
      try {
        await lx.showActionSheet({ items: [] });
        return { ok: true };
      } catch (error) {
        const { code, message, data } = error as { code?: string; message?: string; data?: unknown };
        return { ok: false, code: code ?? '', message: String(message ?? error), data: data ?? null };
      }
    });
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
    await app.logic.eval(async ({ lx }) => {
      await lx.tabBar.update({
        visibility: 'auto',
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
    }).catch(() => undefined);
    await app.nav.relaunch({ page: 'home' }).catch(() => undefined);
  });

  await app.nav.relaunch({ page: 'ui', query: { type: 'tabbar' } });
  const ui = await app.page({ name: 'ui' }, { timeout: 30_000 });
  await ui.view.testId('tabbar-show').click({ timeout: 30_000 });
  await waitForTabBar(app, (state) => state.effectiveVisible, 'forced tab bar');

  await t.step('drive showcase badge and red-dot buttons', async () => {
    await ui.view.testId('tabbar-reddot-show').click();
    await waitForTabBar(app, (state) => state.items[1]?.redDot === true, 'red dot from button');
    await ui.view.testId('tabbar-badge-input').fill('9');
    await ui.view.testId('tabbar-badge-set').click();
    await waitForTabBar(app, (state) => state.items[1]?.badge === '9', 'badge from button');
    await ui.view.testId('tabbar-item-text').fill('API+');
    await ui.view.testId('tabbar-item-update').click();
    await waitForTabBar(app, (state) => state.items[1]?.text === 'API+', 'item text from button');
  });

  await t.step('patch the selected tab and a second item together', async () => {
    await app.logic.eval(async ({ lx }) => {
      await lx.tabBar.update({
        items: [
          { index: 0, badge: '1' },
          { index: 1, badge: '2', redDot: false },
        ],
      });
    });
    await waitForTabBar(
      app,
      (state) => state.items[0]?.badge === '1' && state.items[1]?.badge === '2',
      'multi-item tabBar patch',
    );
  });

  await t.step('reject color style patches', async () => {
    const before = await tabBar(app);
    const rejected = await caughtChromeUpdate(app, 'tabBar', { style: { foregroundColor: '#102030' } });
    expect(rejected.ok).toBeFalsy();
    expect(rejected.code).toBe('E_INVALID_ARG');
    expect(await tabBar(app)).toEqual(before);
  });

  await t.step('reject invalid updates with E_INVALID_ARG', async () => {
    const before = await tabBar(app);
    const invalid: JsonValue[] = [
      { items: [{ index: 99, text: 'Invalid' }] },
      { items: [{ index: -1, text: 'Invalid' }] },
      { style: { foregroundColor: 'not-a-color' } },
      { items: [{ index: 1, iconPath: '../secret.png' }] },
    ];
    for (const patch of invalid) {
      const rejected = await caughtChromeUpdate(app, 'tabBar', patch);
      expect(rejected.ok).toBeFalsy();
      expect(rejected.code).toBe('E_INVALID_ARG');
      expect(await tabBar(app)).toEqual(before);
    }
  });

  await t.step('last concurrent update wins without a torn item', async () => {
    await app.logic.eval(async ({ lx }) => {
      await Promise.all([
        lx.tabBar.update({ items: [{ index: 1, text: 'First', badge: 'A' }] }),
        lx.tabBar.update({ items: [{ index: 1, text: 'Second', badge: 'B' }] }),
      ]);
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

  await t.step('switchTab agrees with selectedIndex after a badge change', async () => {
    await app.logic.eval(({ lx }) => {
      void lx.switchTab({ page: 'todo' });
      return 'scheduled';
    });
    await waitForCurrentPage(app, 'todo');
    const state = await waitForTabBar(
      app,
      (item) => item.selectedIndex === 3 && item.routeVisible,
      'todo tab selected',
    );
    expect(state.items[1]?.text === 'API+' || state.items[1]?.text === 'Second' || state.items[1]?.text === 'First')
      .toBeTruthy();
  });
});
