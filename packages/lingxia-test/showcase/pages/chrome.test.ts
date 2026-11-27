import { tabIndex } from '../helpers/tabs.js';
import type { JsonValue, TestApp } from '@lingxia/test';
import type { ToastHandle } from '@lingxia/types';
import { expect, test } from '@lingxia/test';
import type { LxAppRuntimeNavigationBarInfo, LxAppRuntimeTabBarInfo } from '@lingxia/types/automation';
import { bindFixture, eventually, relaunchFromLogic, type Caught } from '../helpers/poll.js';
import { SHOWCASE_APP_ID } from '../helpers/app.js';
import { waitForCurrentPage } from '../helpers/page.js';
import { pinEnglish } from '../helpers/language.js';

// Labels below are the English ones.
test.beforeEach(pinEnglish);

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
    preference: String(lx.host.appearancePreference.get()),
    resolved: String(lx.app.appearance.get()),
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
  return app.logic.eval(async ({ lx, currentPage }, target, patch) => {
    try {
      const bar = target === 'tabBar' ? lx.app.tabBar : currentPage()!.navigationBar;
      await (bar.update as (patch: unknown) => Promise<void>)(patch);
      return { ok: true };
    } catch (error) {
      const { code, message, data } = error as { code?: string; message?: string; data?: unknown };
      return { ok: false, code: code ?? '', message: String(message ?? error), data: data ?? null };
    }
  }, target, patch);
}

test("apply navigationBar title, home button, and reset", {
  id: "UI-NAVBAR-001",
  covers: ['page.navigationBar.update'],
  app: SHOWCASE_APP_ID,
  timeout: 60_000,
}, async (t) => {
  const { app, defer } = bindFixture(t, "UI-NAVBAR-001");
  defer(async () => {
    await app.logic.eval(async ({ currentPage }) => {
      await currentPage()!.navigationBar.update({ title: null, homeButton: 'auto' });
    }).catch(() => undefined);
  });

  const ui = await t.step('set the title from the ui page', async () => {
    await app.nav.reset({ page: 'ui', params: { type: 'navbar' } });
    const page = await app.page({ name: 'ui' }, { timeout: 30_000 });
    await page.view.testId('navbar-title-input').waitFor({ state: 'visible', timeout: 30_000 });
    await page.view.testId('navbar-title-input').fill('Kept Title');
    await page.view.testId('navbar-set-title').click();
    const titled = await waitForNavBar(app, (state) => state.title === 'Kept Title', 'navigationBar title');
    expect(titled.homeButton).toBe('auto');
    return page;
  });

  await t.step('hide the home button', async () => {
    await ui.view.testId('navbar-home-hide').click();
    await waitForNavBar(
      app,
      (state) => state.homeButton === 'hidden' && state.homeButtonVisible === false,
      'hidden home button',
    );
  });

  await t.step('restore the home button from the page control', async () => {
    await ui.view.testId('navbar-home-auto').click();
    await waitForNavBar(app, (state) => state.homeButton === 'auto', 'auto home button');
  });

  await t.step('reject a colour patch without mutating chrome', async () => {
    const before = await navigationBar(app);
    const rejected = await caughtChromeUpdate(app, 'navigationBar', { style: { backgroundColor: '#3B82F6' } });
    expect(rejected.ok).toBeFalsy();
    expect(rejected.code).toBe('E_INVALID_ARG');
    expect(await navigationBar(app)).toEqual(before);
  });

  await t.step('reset the title with null', async () => {
    await ui.view.testId('navbar-reset').click();
    await waitForNavBar(
      app,
      // The manifest title names a catalogue key; either language's text is the reset.
      (state) => (state.title === 'User Interface' || state.title === '界面') && state.homeButton === 'auto',
      'navigationBar reset to manifest title',
    );
  });
});

test("round-trip appearance preference through the ui controls", {
  id: "UI-APPEARANCE-001",
  covers: ['lx.app.appearance.get', 'lx.app.appearance.watch', 'lx.host.appearancePreference.get', 'lx.host.appearancePreference.set'],
  app: SHOWCASE_APP_ID,
  timeout: 60_000,
}, async (t) => {
  const { app, defer } = bindFixture(t, "UI-APPEARANCE-001");
  defer(async () => {
    await app.logic.eval(async ({ lx }) => { await lx.host.appearancePreference.set('auto'); })
      .catch(() => undefined);
  });

  await app.nav.reset({ page: 'ui', params: { type: 'appearance' } });
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
      await expect(ui.view.testId('ui-appearance-preference')).toContainText(state.preference, { timeout: 5_000 });
    });
  }

  await t.step('reject an invalid preference and keep the previous state', async () => {
    const before = await appearanceOf(app);
    const rejected: Caught = await app.logic.eval(async ({ lx }, preference) => {
      try {
        const appearance = lx.host.appearancePreference!;
        await (appearance.set as (preference: unknown) => Promise<void>)(preference);
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
    const relaunched = await app.page({ name: 'ui' });
    await relaunched.view.testId('ui-appearance-dark').waitFor({ state: 'visible', timeout: 30_000 });
    const after = await appearanceOf(app);
    expect(after.preference).toBe('dark');
    expect(after.resolved).toBe('dark');
  });
});

test("show, hide, confirm, and cancel in-app feedback overlays", {
  id: "UI-FEEDBACK-001",
  covers: ['lx.ui.toast', 'lx.ui.alert', 'lx.ui.confirm', 'lx.ui.actionSheet'],
  app: SHOWCASE_APP_ID,
  timeout: 60_000,
}, async (t) => {
  const { app } = bindFixture(t, "UI-FEEDBACK-001");
  // Every host draws Logic's feedback on the layer over the app's window.
  const layer = app.feedback.view;

  await t.step('show and hide a toast', async () => {
    await app.nav.reset({ page: 'ui', params: { type: 'toast' } });
    const page = await app.page({ name: 'ui' }, { timeout: 30_000 });
    await page.view.testId('toast-show').waitFor({ state: 'visible', timeout: 30_000 });
    // The handle stays in Logic: it is not JSON.
    await app.logic.eval(async ({ lx }) => {
      (globalThis as { coverageToast?: ToastHandle }).coverageToast =
        lx.ui.toast('Coverage toast', { durationMs: 8000 });
    });
    await expect(layer.testId('lx-toast')).toContainText('Coverage toast', { timeout: 30_000 });
    // The layer is over the page, not in it.
    await expect(page.view.testId('lx-toast')).toHaveCount(0);
    await app.logic.eval(async () => {
      (globalThis as { coverageToast?: ToastHandle }).coverageToast?.dismiss();
    });
    await expect(layer.testId('lx-toast')).toHaveCount(0, { timeout: 10_000 });
  });

  const press = async (testId: string) => {
    await layer.testId(testId).first().waitFor({ state: 'visible', timeout: 30_000 });
    await layer.testId(testId).click();
  };

  await t.step('confirm and cancel a modal', async () => {
    const confirmed = app.logic.eval(({ lx }) =>
      lx.ui.confirm({ title: 'Coverage', message: 'Confirm this', confirmText: 'OK', cancelText: 'Cancel' }));
    await press('lx-modal-confirm');
    expect(await confirmed).toBe(true);

    const canceled = app.logic.eval(({ lx }) =>
      lx.ui.confirm({ title: 'Coverage', message: 'Cancel this' }));
    await press('lx-modal-cancel');
    expect(await canceled).toBe(false);

    const noCancel = app.logic.eval(async ({ lx }) => { await lx.ui.alert('No cancel'); return true; });
    await layer.testId('lx-modal-confirm').first().waitFor({ state: 'visible', timeout: 30_000 });
    await expect(layer.testId('lx-modal-cancel')).toHaveCount(0);
    await press('lx-modal-confirm');
    expect(await noCancel).toBe(true);
  });

  await t.step('abort a confirm with its signal', async () => {
    type Held = { coverageAbort?: AbortController };
    const aborted = app.logic.eval(async ({ lx }) => {
      const controller = new AbortController();
      (globalThis as Held).coverageAbort = controller;
      try {
        await lx.ui.confirm({ title: 'Abort me', signal: controller.signal });
        return 'settled';
      } catch (error) {
        return (error as { code?: string }).code ?? String(error);
      }
    });
    await layer.testId('lx-modal-confirm').first().waitFor({ state: 'visible', timeout: 30_000 });
    await app.logic.eval(() => {
      const held = globalThis as Held;
      const controller = held.coverageAbort;
      delete held.coverageAbort;
      controller?.abort();
    });
    expect(await aborted).toBe('E_ABORT');
    // The dialog closes with the call: nothing is left for the user to answer.
    await expect(layer.testId('lx-modal-confirm')).toHaveCount(0, { timeout: 10_000 });
  });

  await t.step('use acknowledgement and boolean dialogs', async () => {
    const alert = app.logic.eval(async ({ lx }) => {
      await lx.ui.alert({ title: 'Notice' });
      return 'acknowledged';
    });
    await press('lx-modal-confirm');
    expect(await alert).toBe('acknowledged');
    const confirmed = app.logic.eval(({ lx }) => lx.ui.confirm({ title: 'Continue?' }));
    await press('lx-modal-confirm');
    expect(await confirmed).toBe(true);
    const canceled = app.logic.eval(({ lx }) => lx.ui.confirm({ title: 'Continue?' }));
    await press('lx-modal-cancel');
    expect(await canceled).toBe(false);
  });

  await t.step('pick and dismiss an action sheet', async () => {
    const picked = app.logic.eval(({ lx }) =>
      lx.ui.actionSheet({ items: ['View Details', '查看日志', 'Send Email', '删除'].map((label) => ({ id: label, label })) }));
    // The layer covers the page area, never a sidebar beside it: as wide as the page, and between
    // the page and the window in height (a page card may include its navigation bar).
    await layer.testId('lx-action-sheet').first().waitFor({ state: 'visible', timeout: 30_000 });
    const covered = await layer.testId('lx-feedback').boundingBox();
    const pageSize = await (await app.page({ name: 'ui' })).view.viewport();
    const window = await app.chrome.viewport().catch(() => null);
    expect(Math.abs(covered.width - pageSize.width), `layer ${covered.width} wide over a ${pageSize.width} wide page`).toBeLessThanOrEqual(1);
    expect(covered.height).toBeGreaterThanOrEqual(pageSize.height - 1);
    if (window) expect(covered.height).toBeLessThanOrEqual(window.height + 1);
    await press('lx-action-sheet-item-1');
    const selected = await picked;
    expect(selected.status === 'canceled').toBeFalsy();
    expect(selected.status === 'ok' ? selected.id : null).toBe('查看日志');

    const dismissed = app.logic.eval(({ lx }) =>
      lx.ui.actionSheet({ items: ['One', 'Two'].map((label) => ({ id: label, label })) }));
    await press('lx-action-sheet-cancel');
    expect((await dismissed).status === 'canceled').toBeTruthy();
  });

  await t.step('reject an empty action sheet', async () => {
    const rejected: Caught = await app.logic.eval(async ({ lx }) => {
      try {
        await lx.ui.actionSheet({ items: [] });
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

test("assert tabBar failure codes, resets, and button-driven patches", {
  id: "UI-TABBAR-002",
  covers: ['lx.app.tabBar', 'lx.app.tabBar.update', 'lx.nav.go'],
  app: SHOWCASE_APP_ID,
  timeout: 60_000,
}, async (t) => {
  const { app, defer } = bindFixture(t, "UI-TABBAR-002");
  defer(async () => {
    await app.logic.eval(async ({ lx }) => {
      await lx.app.tabBar.update({
        visibility: 'auto',
        items: {
          home: { label: null, icon: null, badge: null },
          api: { label: null, icon: null, badge: null },
        },
      });
    }).catch(() => undefined);
    await app.nav.reset({ page: 'home' }).catch(() => undefined);
  });

  await app.nav.reset({ page: 'ui', params: { type: 'tabbar' } });
  const ui = await app.page({ name: 'ui' }, { timeout: 30_000 });
  await ui.view.testId('tabbar-show').click({ timeout: 30_000 });
  await waitForTabBar(app, (state) => state.effectiveVisible, 'forced tab bar');
  const todoIndex = await tabIndex(app, ['ToDo', '待办']);

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
      await lx.app.tabBar.update({
        items: {
          home: { badge: '1' },
          api: { badge: '2' },
        },
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
      { items: { 'no-such-tab': { label: 'Invalid' } } },
      { items: [{ index: 1, label: 'Invalid' }] },
      { items: { api: { icon: '../secret.png' } } },
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
        lx.app.tabBar.update({ items: { api: { label: 'First', badge: 'A' } } }),
        lx.app.tabBar.update({ items: { api: { label: 'Second', badge: 'B' } } }),
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
      void lx.nav.go({ tab: 'todo' });
      return 'scheduled';
    });
    await waitForCurrentPage(app, 'todo');
    const state = await waitForTabBar(
      app,
      (item) => item.selectedIndex === todoIndex && item.effectiveVisible,
      'todo tab selected',
    );
    expect(state.items[1]?.text === 'API+' || state.items[1]?.text === 'Second' || state.items[1]?.text === 'First')
      .toBeTruthy();
  });
});
