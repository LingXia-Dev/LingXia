import { expect, test } from '@rongjs/test';
import type { LxAppDriver } from 'lingxia-types';

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
  const deadline = Date.now() + 10_000;
  let state = await refreshState(app);
  while (Date.now() < deadline) {
    if (predicate(state)) return state;
    await new Promise((resolve) => setTimeout(resolve, 50));
    state = await refreshState(app);
  }
  throw new Error(`Timed out waiting for pull-to-refresh state: ${JSON.stringify(state)}`);
}

async function waitForStatus(app: LxAppDriver, expected: string): Promise<string> {
  const deadline = Date.now() + 10_000;
  let text = '';
  while (Date.now() < deadline) {
    const status = await app.page.query({
      page: 'pullToRefresh',
      css: '[data-testid="pull-refresh-status"]',
      full: true,
    });
    text = status.exists ? status.text : '';
    if (text.includes(expected)) return text;
    await new Promise((resolve) => setTimeout(resolve, 50));
  }
  throw new Error(`Timed out waiting for pull-to-refresh status '${expected}', received '${text}'`);
}

test('starts, receives, renders, and stops the native pull-to-refresh lifecycle', async () => {
  const app = lx.automation().lxapp();
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
