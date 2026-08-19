import type { LxAppDriver } from 'lingxia-types/automation';
import { waitForElementText } from '../helpers/page.js';
import { expect, spec } from '@lingxia/test';
import { bindFixture, eventually, specNamespace } from '../helpers/poll.js';
import { SHOWCASE_APP_ID } from '../helpers/app.js';

interface RefreshState {
  count: number;
  refreshing: boolean;
}

async function refreshState(app: LxAppDriver): Promise<RefreshState> {
  return app.eval({
    script: `
      const page = getCurrentPages().find((candidate) => candidate.route.includes('/pulltorefresh/'));
      return { count: page?.data?.refreshCount ?? -1, refreshing: !!page?.data?.isRefreshing };
    `,
  }) as Promise<RefreshState>;
}

async function waitForRefreshState(
  app: LxAppDriver,
  predicate: (state: RefreshState) => boolean,
): Promise<RefreshState> {
  return eventually(refreshState.bind(null, app), predicate, {
    describe: 'pull-to-refresh Logic state',
    timeoutMs: 30_000,
  });
}

async function waitForStatus(app: LxAppDriver, expected: string): Promise<string> {
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
  await app.page.click({ page: 'pullToRefresh', css: '[data-testid="pull-refresh-start"]' });
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

  await app.page.click({ page: 'pullToRefresh', css: '[data-testid="pull-refresh-stop"]' });
  await waitForRefreshState(app, (state) => !state.refreshing && state.count === refreshing.count);
  expect(await waitForStatus(app, 'Idle')).toContain('Idle');
});
