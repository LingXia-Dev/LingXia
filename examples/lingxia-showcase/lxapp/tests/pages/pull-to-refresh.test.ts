import type { Fixture, TestApp, TestView } from '@lingxia/test';
import { waitForElementText } from '../helpers/page.js';
import { expect, spec } from '@lingxia/test';
import { bindFixture, eventually, type Caught } from '../helpers/poll.js';
import { SHOWCASE_APP_ID } from '../helpers/app.js';

interface RefreshState {
  count: number;
  refreshing: boolean;
}

async function refreshState(app: TestApp): Promise<RefreshState> {
  return app.logic.eval(({ getCurrentPages }): RefreshState => {
    const page = getCurrentPages().find((candidate) => candidate.route.includes('/pulltorefresh/'));
    const data = (page?.data ?? {}) as { refreshCount?: number; isRefreshing?: boolean };
    return { count: data.refreshCount ?? -1, refreshing: !!data.isRefreshing };
  });
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

async function waitForStatus(t: Fixture, view: TestView, expected: string): Promise<string> {
  return waitForElementText(
    t,
    view,
    '[data-testid="pull-refresh-status"]',
    (text) => text.includes(expected),
    30_000,
  );
}

spec("start, render, and stop the native pull-to-refresh lifecycle", { id: "PULL-001", covers: ['lx.startPullDownRefresh', 'lx.stopPullDownRefresh'], app: SHOWCASE_APP_ID }, async (t) => {
  const { app } = bindFixture(t, "PULL-001");

  await app.nav.relaunch({ page: 'pullToRefresh' });
  const pull = (await app.page({ name: 'pullToRefresh' }, { timeout: 30_000 })).view;
  await pull.testId('pull-refresh-page').waitFor({ timeout: 30_000 });

  const before = await refreshState(app);
  await pull.testId("pull-refresh-start").click();
  const refreshing = await waitForRefreshState(
    app,
    (state) => state.refreshing && state.count > before.count,
  );
  expect(await waitForStatus(t, pull, 'Refreshing')).toContain('Refreshing');
  await expect(pull.testId('pull-refresh-count')).toHaveText(String(refreshing.count));

  await pull.testId("pull-refresh-stop").click();
  await waitForRefreshState(app, (state) => !state.refreshing && state.count === refreshing.count);
  expect(await waitForStatus(t, pull, 'Idle')).toContain('Idle');

  await t.step('start rejects when the current page has not enabled pull-down refresh', async () => {
    await app.nav.relaunch({ page: 'home' });
    // The relaunch waited for home to be ready, so it is the current page.
    await app.view.testId('home-page').waitFor({ timeout: 30_000 });

    const rejected: Caught = await app.logic.eval(async ({ lx }) => {
      try {
        await lx.startPullDownRefresh();
        return { ok: true };
      } catch (error) {
        const { code, message, data } = error as { code?: string; message?: string; data?: unknown };
        return { ok: false, code: code ?? '', message: String(message ?? error), data: data ?? null };
      }
    });
    expect(rejected.ok).toBeFalsy();
    expect(rejected.code).toBe('E_INVALID_STATE');
    expect((rejected.data as { bizCode?: number } | undefined)?.bizCode).toBe(4004);
    expect((rejected.data as { detail?: string } | undefined)?.detail)
      .toContain('enablePullDownRefresh: true');
  });
});
