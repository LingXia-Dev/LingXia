import type { TestApp } from '@lingxia/test';
import { waitForElementText } from '../helpers/page.js';
import { expect, spec } from '@lingxia/test';
import { bindFixture, evalCaught, eventually, specNamespace } from '../helpers/poll.js';
import { SHOWCASE_APP_ID } from '../helpers/app.js';

interface RefreshState {
  count: number;
  refreshing: boolean;
}

async function refreshState(app: TestApp): Promise<RefreshState> {
  return app.eval({
    script: `
      const page = getCurrentPages().find((candidate) => candidate.route.includes('/pulltorefresh/'));
      return { count: page?.data?.refreshCount ?? -1, refreshing: !!page?.data?.isRefreshing };
    `,
  }) as Promise<RefreshState>;
}

async function waitForRefreshState(
  app: TestApp,
  predicate: (state: RefreshState) => boolean,
): Promise<RefreshState> {
  return eventually(refreshState.bind(null, app), predicate, {
    describe: 'pull-to-refresh Logic state',
    timeoutMs: 30_000,
  });
}

async function waitForStatus(app: TestApp, expected: string): Promise<string> {
  return waitForElementText(
    app,
    'pullToRefresh',
    '[data-testid="pull-refresh-status"]',
    (text) => text.includes(expected),
    30_000,
  );
}

spec("start, render, and stop the native pull-to-refresh lifecycle", { id: "PULL-001", covers: ['lx.startPullDownRefresh', 'lx.stopPullDownRefresh'], app: SHOWCASE_APP_ID }, async (t) => {
  const { app } = bindFixture(t, "PULL-001");

  await app.nav.relaunch({ page: 'pullToRefresh' });
  await app.page.waitFor({ page: 'pullToRefresh', css: '[data-testid="pull-refresh-page"]' });

  const before = await refreshState(app);
  await app.view.testId("pull-refresh-start", { page: 'pullToRefresh' }).click();
  const refreshing = await waitForRefreshState(
    app,
    (state) => state.refreshing && state.count > before.count,
  );
  expect(await waitForStatus(app, 'Refreshing')).toContain('Refreshing');

  const count = await app.page.query({
    page: 'pullToRefresh',
    css: '[data-testid="pull-refresh-count"]',
    full: true,
  });
  expect(count.exists && Number(count.text)).toBe(refreshing.count);

  await app.view.testId("pull-refresh-stop", { page: 'pullToRefresh' }).click();
  await waitForRefreshState(app, (state) => !state.refreshing && state.count === refreshing.count);
  expect(await waitForStatus(app, 'Idle')).toContain('Idle');

  await t.step('start rejects when the current page has not enabled pull-down refresh', async () => {
    await app.nav.relaunch({ page: 'home' });
    await app.page.waitFor({ page: 'home', css: '[data-testid="home-page"]' });

    const rejected = await evalCaught(app, 'lx.startPullDownRefresh();');
    expect(rejected.ok).toBeFalsy();
    expect(rejected.code).toBe('E_INVALID_STATE');
    expect((rejected.data as { bizCode?: number } | undefined)?.bizCode).toBe(4004);
    expect((rejected.data as { detail?: string } | undefined)?.detail)
      .toContain('enablePullDownRefresh: true');
  });
});
