import type { PageInfo } from '@lingxia/types/automation';
import type { PageDefinition } from '@lingxia/types/page';
import { currentPageOrNull } from '@lingxia/test/showcase/helpers/page.js';
import { expect, test, type AnyLogicPage, type TestApp } from '@lingxia/test';
import { bindFixture, eventually, relaunchFromLogic, specNamespace, type Caught } from '@lingxia/test/showcase/helpers/poll.js';
import { SHOWCASE_APP_ID } from '@lingxia/test/showcase/helpers/app.js';


/** The device page's section, which its `type` query picks on load. */
async function deviceSection(app: TestApp, expected: string): Promise<void> {
  const device = await app.page<PageDefinition<{ currentType: string }>>({ name: 'device' });
  await device.view.testId('device-page').waitFor({ timeout: 30_000 });
  await expect.poll(async () => (await device.data()).currentType).toBe(expected);
}

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

test("preserve stack, query, redirect, back, and tab semantics", { id: "NAV-001", covers: [
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
  if (initial?.name !== 'home') await app.nav.reset({ page: 'home' });
  await waitForCurrent(app, 'home');
  expect((await app.nav.stack()).map((page) => page.name)).toEqual(['home']);

  await app.nav.go({ page: 'device', params: { type: 'screen' } });
  await waitForCurrent(app, 'device');
  await deviceSection(app, 'screen');
  expect((await app.nav.stack()).map((page) => page.name)).toEqual(['home', 'device']);

  const backed = await app.nav.back();
  expect(backed.name).toBe('home');
  expect((await app.nav.stack()).map((page) => page.name)).toEqual(['home']);

  await app.nav.go({ page: 'media' });
  await app.nav.go({ page: 'wifi' }, { replace: true });
  await waitForCurrent(app, 'wifi');
  expect((await app.nav.stack()).map((page) => page.name)).toEqual(['home', 'wifi']);

  await app.nav.go({ tab: 'todo' });
  await waitForCurrent(app, 'todo');
  expect((await app.nav.stack()).map((page) => page.name)).toEqual(['todo']);

  // Page instances are cached by path. A query-free navigation must not
  // inherit the query from the earlier device visit.
  await app.nav.reset({ page: 'device' });
  await waitForCurrent(app, 'device');
  // The relaunch opened a new device instance, bound afresh.
  await deviceSection(app, 'device');
  expect((await app.nav.stack()).map((page) => page.name)).toEqual(['device']);
});

test("reset from Logic and reject invalid navigation", {
  id: "NAV-LOGIC-001",
  covers: ['lx.nav.reset', 'lx.nav.go', 'lx.nav.back'],
  app: SHOWCASE_APP_ID,
  timeout: 90_000,
}, async (t) => {
  const { app } = bindFixture(t, "NAV-LOGIC-001");

  await t.step('lx.nav.reset replaces the stack', async () => {
    await app.nav.reset({ page: 'home' });
    await waitForCurrent(app, 'home');
    await relaunchFromLogic(app, 'device', { type: 'screen' });
    await waitForCurrent(app, 'device');
    expect((await app.nav.stack()).map((page) => page.name)).toEqual(['device']);
  });

  await t.step('unknown page rejects', async () => {
    const rejected = await app.logic.eval(async ({ lx }): Promise<Caught> => {
      try {
        await lx.nav.go({ page: 'does-not-exist' }).opened;
        return { ok: true };
      } catch (error) {
        const { code, message, data } = error as { code?: string; message?: string; data?: unknown };
        return { ok: false, code: code ?? '', message: String(message ?? error), data: data ?? null };
      }
    });
    expect(rejected.ok).toBeFalsy();
    expect(rejected.code).toBe('E_NOT_FOUND');
    expect((await app.nav.stack()).map((page) => page.name)).toEqual(['device']);
  });

  await t.step('pushing a tab already on the stack is duplicate_route', async () => {
    await relaunchFromLogic(app, 'home');
    await waitForCurrent(app, 'home');
    const rejected = await app.logic.eval(async ({ lx }): Promise<Caught> => {
      try {
        await lx.nav.go({ page: 'home' }).opened;
        return { ok: true };
      } catch (error) {
        const { code, message, data } = error as { code?: string; message?: string; data?: unknown };
        return { ok: false, code: code ?? '', message: String(message ?? error), data: data ?? null };
      }
    });
    expect(rejected.ok).toBeFalsy();
    expect(rejected.code).toBe('E_INVALID_ARG');
    const reason = (rejected.data as { reason?: string } | undefined)?.reason;
    expect(reason).toBe('duplicate_route');
  });

  await t.step('back at the root is a no-op', async () => {
    const before = (await app.nav.stack()).map((page) => page.name);
    const result = await app.logic.eval(async ({ lx }): Promise<Caught> => {
      try {
        await lx.nav.back(1);
        return { ok: true };
      } catch (error) {
        const { code, message, data } = error as { code?: string; message?: string; data?: unknown };
        return { ok: false, code: code ?? '', message: String(message ?? error), data: data ?? null };
      }
    });
    expect(result.ok).toBeTruthy();
    expect((await app.nav.stack()).map((page) => page.name)).toEqual(before);
  });

  await t.step('replacing the current route keeps stack length 1', async () => {
    await app.nav.reset({ page: 'ui', params: { type: 'navigation' } });
    await waitForCurrent(app, 'ui');
    await app.logic.eval(({ lx }) => {
      void lx.nav.go({ page: 'ui', params: { type: 'navbar' } }, { replace: true });
      return 'scheduled';
    });
    // `ui` is already current, so waiting on the route alone would pass before
    // the scheduled redirect lands and leave it in flight for the next case.
    // The page's own query is what proves it arrived.
    await eventually(
      () => app.logic.eval(({ pageStack }) => {
        const page = pageStack().find((candidate) => candidate.route.includes('/ui/'));
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
  return app.logic.eval(({ pageStack }) => {
    const page = pageStack<AnyLogicPage<{ loadCount?: number }>>()
      .find((candidate) => candidate.route.includes('/api/'));
    return page
      ? { loadCount: page.data.loadCount ?? -1, logicMarker: (page as unknown as { __relaunchMarker?: string }).__relaunchMarker ?? null }
      : null;
  });
}

/** Visit the api tab, then mark its Logic page object. */
async function markWarmApiTab(app: TestApp, marker: string): Promise<PageInfo> {
  await app.nav.reset({ page: 'home' });
  await waitForCurrent(app, 'home');
  await app.nav.go({ tab: 'api' });
  const warm = await waitForCurrent(app, 'api');
  await eventually(() => apiPageProbe(app), (probe) => probe?.loadCount === 1, {
    describe: 'the api tab to run onLoad once',
  });
  await app.logic.eval(({ pageStack }, marker) => {
    const page = pageStack().find((candidate) => candidate.route.includes('/api/'));
    // A marker on the page object itself: a fresh instance will not carry it.
    (page as unknown as { __relaunchMarker?: string }).__relaunchMarker = marker;
    return true;
  }, marker);
  // Leave it warm off the stack: switchTab keeps tab pages alive.
  await app.nav.go({ tab: 'home' });
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
  const api = await app.page({ name: 'api' });
  expect(api.instanceId).toBe(current.instanceId);
  expect((await app.nav.stack()).map((page) => page.name)).toEqual(['api']);
}

test("relaunch onto a warm tab page opens a fresh instance", {
  id: "NAV-RELAUNCH-001",
  covers: ['NavDriver.relaunch', 'NavDriver.switchTab', 'lx.nav.reset'],
  app: SHOWCASE_APP_ID,
  timeout: 90_000,
}, async (t) => {
  const { app, defer } = bindFixture(t, "NAV-RELAUNCH-001");
  defer(async () => {
    await app.nav.reset({ page: 'home' });
  });

  await t.step('t.app.nav.relaunch from another tab', async () => {
    const stale = await markWarmApiTab(app, 'nav-relaunch');
    const landed = await app.nav.reset({ page: 'api' });
    await expectFreshApiTab(app, stale, landed);
  });

  await t.step('lx.nav.reset from another tab', async () => {
    const stale = await markWarmApiTab(app, 'lx-relaunch');
    await relaunchFromLogic(app, 'api');
    await expectFreshApiTab(app, stale);
  });

  await t.step('a warm tab that was not the target starts over too', async () => {
    await markWarmApiTab(app, 'other-target');
    await app.nav.reset({ page: 'device' });
    await waitForCurrent(app, 'device');
    await app.nav.go({ tab: 'api' });
    await waitForCurrent(app, 'api');
    const probe = await eventually(() => apiPageProbe(app), (value) => value !== null && value.loadCount > 0, {
      describe: 'the api tab to run onLoad again',
    });
    expect(probe).toEqual({ loadCount: 1, logicMarker: null });
  });
});
