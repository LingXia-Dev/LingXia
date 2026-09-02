import type { LxAppDriver, PageInfo } from 'lingxia-types/automation';
import { currentPageOrNull, waitForElementAttribute } from '../helpers/page.js';
import { expect, spec } from '@lingxia/test';
import { bindFixture, evalCaught, eventually, relaunchFromLogic, specNamespace } from '../helpers/poll.js';
import { SHOWCASE_APP_ID } from '../helpers/app.js';

async function waitForCurrent(app: LxAppDriver, name: string): Promise<PageInfo> {
  return eventually(
    () => app.nav.current(),
    (current) => current.name === name && current.ready,
    { describe: `current page '${name}' to become ready`, timeoutMs: 30_000 });
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
  await app.page.waitFor({ page: 'device', css: '[data-testid="device-page"]' });
  await waitForElementAttribute(
    app,
    'device',
    '[data-testid="device-page"]',
    'data-mode',
    'screen',
  );
  const mode = await app.page.eval({
    page: 'device',
    script: 'document.querySelector(\'[data-testid="device-page"]\')?.getAttribute(\'data-mode\')',
  });
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
  await app.page.waitFor({ page: 'device', css: '[data-testid="device-page"]' });
  await waitForElementAttribute(
    app,
    'device',
    '[data-testid="device-page"]',
    'data-mode',
    'device',
  );
  const defaultMode = await app.page.eval({
    page: 'device',
    script: 'document.querySelector(\'[data-testid="device-page"]\')?.getAttribute(\'data-mode\')',
  });
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
    const rejected = await evalCaught(app, `await lx.navigateTo({ page: 'does-not-exist' });`);
    expect(rejected.ok).toBeFalsy();
    expect(rejected.code).toBe('E_NOT_FOUND');
    expect((await app.nav.stack()).map((page) => page.name)).toEqual(['device']);
  });

  await t.step('navigateTo a tab already on the stack is duplicate_route', async () => {
    await relaunchFromLogic(app, 'home');
    await waitForCurrent(app, 'home');
    const rejected = await evalCaught(app, `await lx.navigateTo({ page: 'home' });`);
    expect(rejected.ok).toBeFalsy();
    expect(rejected.code).toBe('E_INVALID_ARG');
    const reason = (rejected.data as { reason?: string } | undefined)?.reason;
    expect(reason).toBe('duplicate_route');
  });

  await t.step('navigateBack at root is a no-op', async () => {
    const before = (await app.nav.stack()).map((page) => page.name);
    const result = await evalCaught(app, `await lx.navigateBack({ delta: 1 });`);
    expect(result.ok).toBeTruthy();
    expect((await app.nav.stack()).map((page) => page.name)).toEqual(before);
  });

  await t.step('redirectTo the current route keeps stack length 1', async () => {
    await app.nav.relaunch({ page: 'ui', query: { type: 'navigation' } });
    await waitForCurrent(app, 'ui');
    await app.eval({
      script: `void lx.redirectTo({ page: 'ui', query: { type: 'navbar' } }); return 'scheduled';`,
    });
    // `ui` is already current, so waiting on the route alone would pass before
    // the scheduled redirect lands and leave it in flight for the next case.
    // The page's own query is what proves it arrived.
    await eventually(
      () => app.eval({
        script: `
          const page = getCurrentPages().find((candidate) => candidate.route.includes('/ui/'));
          return String(page?.data?.currentType ?? '');
        `,
      }),
      (currentType) => currentType === 'navbar',
      { describe: 'the scheduled redirect to land on the ui page', timeoutMs: 15_000 },
    );
    await waitForCurrent(app, 'ui');
    expect((await app.nav.stack()).map((page) => page.name)).toEqual(['ui']);
  });
});


