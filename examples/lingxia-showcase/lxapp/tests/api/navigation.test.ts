import type { PageInfo } from '@lingxia/types/automation';
import { currentPageOrNull, waitForElementAttribute } from '../helpers/page.js';
import { expect, spec, type AnyLogicPage, type TestApp } from '@lingxia/test';
import { bindFixture, eventually, relaunchFromLogic, specNamespace, type Caught } from '../helpers/poll.js';
import { SHOWCASE_APP_ID } from '../helpers/app.js';
import type { ProbeWindow } from '../helpers/view.js';


async function waitForCurrent(app: TestApp, name: string): Promise<PageInfo> {
  return eventually(
    () => app.nav.current(),
    (current) => current.name === name && current.ready,
    {
      describe: `current page '${name}' to become ready`,
      timeoutMs: 30_000,
      // A relaunch scheduled from Logic empties the stack before it lands.
      retryIf: (error) => (error as { code?: string }).code === 'E_PAGE_NOT_READY',
    });
}

spec("preserve stack, query, redirect, back, and tab semantics", { id: "NAV-001", covers: [
    'NavDriver.relaunch',
    'NavDriver.to',
    'NavDriver.current',
    'NavDriver.stack',
    'NavDriver.back',
    'NavDriver.redirect',
    'NavDriver.switchTab',
  ], app: SHOWCASE_APP_ID }, async (t) => {
  const { app } = bindFixture(t, "NAV-001");

  const initial = await currentPageOrNull(app);
  if (initial?.name !== 'home') await app.nav.relaunch({ page: 'home' });
  await waitForCurrent(app, 'home');
  expect((await app.nav.stack()).map((page) => page.name)).toEqual(['home']);

  await app.nav.to({ page: 'device', query: { type: 'screen' } });
  await waitForCurrent(app, 'device');
  await app.view.testId('device-page', { page: 'device' }).waitFor({ timeout: 30_000 });
  await waitForElementAttribute(
    t,
    'device',
    '[data-testid="device-page"]',
    'data-mode',
    'screen',
  );
  const mode = await app.view.eval({ page: 'device' }, ({ document }) => document.querySelector('[data-testid="device-page"]')?.getAttribute('data-mode'));
  expect(mode).toBe('screen');
  expect((await app.nav.stack()).map((page) => page.name)).toEqual(['home', 'device']);

  const backed = await app.nav.back();
  expect(backed.name).toBe('home');
  expect((await app.nav.stack()).map((page) => page.name)).toEqual(['home']);

  await app.nav.to({ page: 'components' });
  await app.nav.redirect({ page: 'picker' });
  await waitForCurrent(app, 'picker');
  expect((await app.nav.stack()).map((page) => page.name)).toEqual(['home', 'picker']);

  await app.nav.switchTab({ page: 'todo' });
  await waitForCurrent(app, 'todo');
  expect((await app.nav.stack()).map((page) => page.name)).toEqual(['todo']);

  // Page instances are cached by path. A query-free navigation must not
  // inherit the query from the earlier device visit.
  await app.nav.relaunch({ page: 'device' });
  await waitForCurrent(app, 'device');
  await app.view.testId('device-page', { page: 'device' }).waitFor({ timeout: 30_000 });
  await waitForElementAttribute(
    t,
    'device',
    '[data-testid="device-page"]',
    'data-mode',
    'device',
  );
  const defaultMode = await app.view.eval({ page: 'device' }, ({ document }) => document.querySelector('[data-testid="device-page"]')?.getAttribute('data-mode'));
  expect(defaultMode).toBe('device');
  expect((await app.nav.stack()).map((page) => page.name)).toEqual(['device']);
});

spec("reLaunch from Logic and reject invalid navigation", {
  id: "NAV-LOGIC-001",
  covers: ['lx.reLaunch', 'lx.navigateTo', 'lx.navigateBack', 'lx.redirectTo', 'lx.switchTab'],
  app: SHOWCASE_APP_ID,
  timeout: 90_000,
}, async (t) => {
  const { app } = bindFixture(t, "NAV-LOGIC-001");

  await t.step('lx.reLaunch replaces the stack', async () => {
    await app.nav.relaunch({ page: 'home' });
    await waitForCurrent(app, 'home');
    await relaunchFromLogic(app, 'device', { type: 'screen' });
    await waitForCurrent(app, 'device');
    expect((await app.nav.stack()).map((page) => page.name)).toEqual(['device']);
  });

  await t.step('unknown page rejects', async () => {
    const rejected = await app.logic.eval(async ({ lx }): Promise<Caught> => {
      try {
        await lx.navigateTo({ page: 'does-not-exist' });
        return { ok: true };
      } catch (error) {
        const { code, message, data } = error as { code?: string; message?: string; data?: unknown };
        return { ok: false, code, message: String(message ?? error), data };
      }
    });
    expect(rejected.ok).toBeFalsy();
    expect(rejected.code).toBe('E_NOT_FOUND');
    expect((await app.nav.stack()).map((page) => page.name)).toEqual(['device']);
  });

  await t.step('navigateTo a tab already on the stack is duplicate_route', async () => {
    await relaunchFromLogic(app, 'home');
    await waitForCurrent(app, 'home');
    const rejected = await app.logic.eval(async ({ lx }): Promise<Caught> => {
      try {
        await lx.navigateTo({ page: 'home' });
        return { ok: true };
      } catch (error) {
        const { code, message, data } = error as { code?: string; message?: string; data?: unknown };
        return { ok: false, code, message: String(message ?? error), data };
      }
    });
    expect(rejected.ok).toBeFalsy();
    expect(rejected.code).toBe('E_INVALID_ARG');
    const reason = (rejected.data as { reason?: string } | undefined)?.reason;
    expect(reason).toBe('duplicate_route');
  });

  await t.step('navigateBack at root is a no-op', async () => {
    const before = (await app.nav.stack()).map((page) => page.name);
    const result = await app.logic.eval(async ({ lx }): Promise<Caught> => {
      try {
        await lx.navigateBack({ delta: 1 });
        return { ok: true };
      } catch (error) {
        const { code, message, data } = error as { code?: string; message?: string; data?: unknown };
        return { ok: false, code, message: String(message ?? error), data };
      }
    });
    expect(result.ok).toBeTruthy();
    expect((await app.nav.stack()).map((page) => page.name)).toEqual(before);
  });

  await t.step('redirectTo the current route keeps stack length 1', async () => {
    await app.nav.relaunch({ page: 'ui', query: { type: 'navigation' } });
    await waitForCurrent(app, 'ui');
    await app.logic.eval(({ lx }) => {
      void lx.redirectTo({ page: 'ui', query: { type: 'navbar' } });
      return 'scheduled';
    });
    // `ui` is already current, so waiting on the route alone would pass before
    // the scheduled redirect lands and leave it in flight for the next case.
    // The page's own query is what proves it arrived.
    await eventually(
      () => app.logic.eval(({ getCurrentPages }) => {
        const page = getCurrentPages().find((candidate) => candidate.route.includes('/ui/'));
        return String(page?.data?.currentType ?? '');
      }),
      (currentType) => currentType === 'navbar',
      { describe: 'the scheduled redirect to land on the ui page', timeoutMs: 15_000 },
    );
    await waitForCurrent(app, 'ui');
    expect((await app.nav.stack()).map((page) => page.name)).toEqual(['ui']);
  });
});



interface ApiPageProbe {
  loadCount: number;
  logicMarker: string | null;
}

async function apiPageProbe(app: TestApp): Promise<ApiPageProbe | null> {
  return app.logic.eval(({ getCurrentPages }) => {
    const page = getCurrentPages<AnyLogicPage<{ loadCount?: number }>>()
      .find((candidate) => candidate.route.includes('/API/'));
    return page
      ? { loadCount: page.data.loadCount ?? -1, logicMarker: (page.__relaunchMarker as string | undefined) ?? null }
      : null;
  });
}

/** Visit the api tab, then mark its Logic page object and its document. */
async function markWarmApiTab(app: TestApp, marker: string): Promise<PageInfo> {
  await app.nav.relaunch({ page: 'home' });
  await waitForCurrent(app, 'home');
  await app.nav.switchTab({ page: 'api' });
  const warm = await waitForCurrent(app, 'api');
  await eventually(() => apiPageProbe(app), (probe) => probe?.loadCount === 1, {
    describe: 'the api tab to run onLoad once',
  });
  await app.logic.eval(({ getCurrentPages }, marker) => {
    const page = getCurrentPages().find((candidate) => candidate.route.includes('/API/'));
    // A marker on the page object itself: a fresh instance will not carry it.
    (page as unknown as { __relaunchMarker?: string }).__relaunchMarker = marker;
    return true;
  }, marker);
  await app.view.eval({ page: 'api' }, ({ window }, marker) => {
    (window as ProbeWindow).__relaunchMarker = marker;
    return true;
  }, marker);
  // Leave it warm off the stack: switchTab keeps tab pages alive.
  await app.nav.switchTab({ page: 'home' });
  await waitForCurrent(app, 'home');
  return warm;
}

async function expectFreshApiTab(app: TestApp, stale: PageInfo, landed?: PageInfo): Promise<void> {
  // A relaunch scheduled from Logic empties the stack before it lands.
  const current = await eventually(
    () => app.nav.current(),
    (page) => page.name === 'api' && page.ready,
    {
      describe: "the relaunched page 'api' to become ready",
      timeoutMs: 30_000,
      retryIf: (error) => (error as { code?: string }).code === 'E_PAGE_NOT_READY',
    },
  );
  expect(current.instanceId).not.toBe(stale.instanceId);
  if (landed) expect(landed.instanceId).toBe(current.instanceId);
  const probe = await eventually(() => apiPageProbe(app), (value) => value !== null && value.loadCount > 0, {
    describe: 'the relaunched api page to run onLoad',
  });
  expect(probe).toEqual({ loadCount: 1, logicMarker: null });
  const windowMarker = await app.view.eval({ page: 'api' }, ({ window }) => (window as ProbeWindow).__relaunchMarker ?? null);
  expect(windowMarker).toBe(null);
  expect((await app.nav.stack()).map((page) => page.name)).toEqual(['api']);
}

spec("relaunch onto a warm tab page opens a fresh instance", {
  id: "NAV-RELAUNCH-001",
  covers: ['NavDriver.relaunch', 'NavDriver.switchTab', 'lx.reLaunch'],
  app: SHOWCASE_APP_ID,
  timeout: 90_000,
}, async (t) => {
  const { app, defer } = bindFixture(t, "NAV-RELAUNCH-001");
  defer(async () => {
    await app.nav.relaunch({ page: 'home' });
  });

  await t.step('t.app.nav.relaunch from another tab', async () => {
    const stale = await markWarmApiTab(app, 'nav-relaunch');
    const landed = await app.nav.relaunch({ page: 'api' });
    await expectFreshApiTab(app, stale, landed);
  });

  await t.step('lx.reLaunch from another tab', async () => {
    const stale = await markWarmApiTab(app, 'lx-relaunch');
    await relaunchFromLogic(app, 'api');
    await expectFreshApiTab(app, stale);
  });

  await t.step('a warm tab that was not the target starts over too', async () => {
    await markWarmApiTab(app, 'other-target');
    await app.nav.relaunch({ page: 'components' });
    await waitForCurrent(app, 'components');
    await app.nav.switchTab({ page: 'api' });
    await waitForCurrent(app, 'api');
    const probe = await eventually(() => apiPageProbe(app), (value) => value !== null && value.loadCount > 0, {
      describe: 'the api tab to run onLoad again',
    });
    expect(probe).toEqual({ loadCount: 1, logicMarker: null });
    const windowMarker = await app.view.eval({ page: 'api' }, ({ window }) => (window as ProbeWindow).__relaunchMarker ?? null);
    expect(windowMarker).toBe(null);
  });
});
