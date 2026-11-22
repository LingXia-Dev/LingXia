import type { Fixture, TestApp, TestView } from '@lingxia/test';
import { waitForElementText } from '../helpers/page.js';
import { expect, test } from '@lingxia/test';
import { bindFixture, eventually } from '../helpers/poll.js';
import { SHOWCASE_APP_ID } from '../helpers/app.js';

interface RefreshState {
  count: number;
  refreshing: boolean;
}

async function refreshState(app: TestApp): Promise<RefreshState> {
  return app.logic.eval(({ pageStack }): RefreshState => {
    const page = pageStack().find((candidate) => candidate.route.includes('/pulltorefresh/'));
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

test("page.refresh runs onRefresh, and the indicator stays until it settles", { id: "PULL-001", covers: ['page.refresh', 'PageSetup.onRefresh'], app: SHOWCASE_APP_ID }, async (t) => {
  const { app } = bindFixture(t, "PULL-001");

  await app.nav.reset({ page: 'pullToRefresh' });
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

  await t.step('a restart while the stopped indicator still lingers refreshes again', async () => {
    const idle = await refreshState(app);
    await pull.testId("pull-refresh-start").click();
    await waitForRefreshState(app, (state) => state.refreshing && state.count === idle.count + 1);
    // Finish and start again in one Logic turn: the native indicator stays up
    // for a minimum time after a stop, and a start that lands in that window
    // must still reach onRefresh.
    await app.logic.eval(async ({ pageStack, currentPage }) => {
      const pages = pageStack();
      const top = pages[pages.length - 1];
      top.actions.stopRefresh();
      await Promise.resolve();
      await Promise.resolve();
      currentPage()!.refresh();
    });
    const restarted = await waitForRefreshState(
      app,
      (state) => state.refreshing && state.count >= idle.count + 2,
    );
    expect(restarted.count).toBe(idle.count + 2);

    await pull.testId('pull-refresh-stop').click();
    await waitForRefreshState(app, (state) => !state.refreshing && state.count === restarted.count);
    expect(await waitForStatus(t, pull, 'Idle')).toContain('Idle');
  });

  await t.step('refresh throws when the page has not enabled pull-down refresh', async () => {
    await app.nav.reset({ page: 'home' });
    // The relaunch waited for home to be ready, so it is the current page.
    await app.view.testId('home-page').waitFor({ timeout: 30_000 });

    const rejected = await app.logic.eval(async ({ currentPage }) => {
      try {
        currentPage()!.refresh();
        return { ok: true };
      } catch (error) {
        const { code, reason, detail } = error as { code?: string; reason?: string; detail?: string };
        return { ok: false, code: code ?? '', reason: reason ?? '', detail: detail ?? '' };
      }
    });
    expect(rejected.ok).toBeFalsy();
    expect(rejected.code).toBe('E_INVALID_STATE');
    expect(rejected.reason).toBe('refreshNotEnabled');
    expect(rejected.detail).toContain('enablePullDownRefresh: true');
  });
});

/** What the page shows before any pull; a pull or refresh must not take any of it away. */
const PULL_PAGE_IDS = ['pull-refresh-status', 'pull-refresh-count', 'pull-refresh-start', 'pull-refresh-stop'];

/** A point at fractions of the viewport, so a gesture reads the same in either orientation. */
async function at(view: TestView, x: number, y: number): Promise<[number, number]> {
  const { width, height } = await view.viewport();
  return [Math.round(width * x), Math.round(height * y)];
}

async function expectPageIntact(view: TestView, when: string): Promise<void> {
  for (const id of PULL_PAGE_IDS) {
    await expect(view.testId(id), `${id} ${when}`).toBeVisible({ timeout: 3_000 });
  }
}

test("a pull follows the finger, refreshes once past the threshold, and leaves the page whole", { id: "PULL-002", covers: ['page:enablePullDownRefresh'], app: SHOWCASE_APP_ID, requires: { input: ['touch', 'trackpad'], refreshIndicator: true, hostInput: true } }, async (t) => {
  const { app } = bindFixture(t, "PULL-002");
  await app.nav.reset({ page: 'pullToRefresh' });
  const view = (await app.page({ name: 'pullToRefresh' }, { timeout: 30_000 })).view;
  await view.testId('pull-refresh-page').waitFor({ timeout: 30_000 });
  await expectPageIntact(view, 'before the pull');
  const before = await refreshState(app);

  await t.step('the indicator follows the finger and arms past the threshold', async () => {
    const start = await at(view, 0.5, 0.18);
    const finger = await view.touch(start);
    const offsets: number[] = [];
    for (const dy of [40, 80, 120, 160]) {
      await finger.moveTo([start[0], start[1] + dy], { durationMs: 80 });
      offsets.push((await view.refreshIndicator()).offset);
    }
    await t.attach('offsets.json', JSON.stringify(offsets));
    expect(offsets.every((offset, i) => i === 0 || offset > offsets[i - 1]!)).toBe(true);
    expect((await view.refreshIndicator()).phase).toBe('armed');
    await finger.up();
  });

  const refreshing = await waitForRefreshState(app, (state) => state.refreshing && state.count > before.count);
  expect(refreshing.count).toBe(before.count + 1);
  expect(await waitForStatus(t, view, 'Refreshing')).toContain('Refreshing');
  await expect.poll(async () => (await view.refreshIndicator()).phase).toBe('refreshing');

  await t.step('the page stays whole while the refresh holds it open', async () => {
    await t.attach('refreshing.png', await view.screenshot());
    await expectPageIntact(view, 'while refreshing');
    await view.testId('pull-refresh-stop').scrollIntoView();
    await expect(view.testId('pull-refresh-stop')).toBeInViewport();
  });

  // onRefresh fired once, not once per move or on release and again on settle.
  await new Promise<void>((resolve) => setTimeout(resolve, 800));
  expect((await refreshState(app)).count).toBe(before.count + 1);

  await view.testId('pull-refresh-stop').click();
  await waitForRefreshState(app, (state) => !state.refreshing);
  await expect.poll(async () => (await view.refreshIndicator()).phase, { timeout: 3_000 }).toBe('idle');
  await expectPageIntact(view, 'after stop');

  await t.step('a short pull snaps back without refreshing', async () => {
    // A pull starts at the top of the page; Stop may have scrolled it.
    await view.testId('pull-refresh-status').scrollIntoView();
    const short = await view.pullToRefresh({ distance: 30, hold: 100 });
    expect(short.refreshed).toBe(false);
    await expect.poll(async () => (await view.refreshIndicator()).offset, { timeout: 3_000 }).toBe(0);
    expect((await refreshState(app)).count).toBe(before.count + 1);
    await expectPageIntact(view, 'after a short pull');
  });
});

test("a swipe scrolls the page and a pull below the top scrolls back instead of refreshing", { id: "PULL-003", covers: ['view:scroll'], app: SHOWCASE_APP_ID, requires: { input: ['touch', 'trackpad'], refreshIndicator: true } }, async (t) => {
  const { app } = bindFixture(t, "PULL-003");
  await app.nav.reset({ page: 'pullToRefresh' });
  const view = (await app.page({ name: 'pullToRefresh' }, { timeout: 30_000 })).view;
  await view.testId('pull-refresh-page').waitFor({ timeout: 30_000 });
  const status = view.testId('pull-refresh-status');
  const lastRow = view.testId('pull-refresh-row').last();
  await expect(status).toBeInViewport();
  await expect(lastRow).not.toBeInViewport();
  const before = await refreshState(app);

  await t.step('a slow swipe scrolls the top of the page away', async () => {
    const [from, to] = [await at(view, 0.5, 0.68), await at(view, 0.5, 0.29)];
    await view.swipe({ from, to, durationMs: 400 });
    await view.swipe({ from, to, durationMs: 400 });
    await expect(status).not.toBeInViewport();
  });

  await t.step('a downward pull below the top scrolls back and never arms', async () => {
    const finger = await view.touch(await at(view, 0.5, 0.29));
    await finger.moveTo(await at(view, 0.5, 0.45), { durationMs: 200 });
    expect((await view.refreshIndicator()).phase).toBe('idle');
    await finger.up();
    await new Promise<void>((resolve) => setTimeout(resolve, 500));
    expect((await refreshState(app)).count).toBe(before.count);
  });
});

// Touch only: synthesized trackpad momentum does not carry a macOS WebView,
// so a desktop fling would measure the injector, not the page.
test("a fling carries the page past where the finger stopped", { id: "PULL-004", covers: ['view:scroll'], app: SHOWCASE_APP_ID, requires: { input: ['touch'] } }, async (t) => {
  const { app } = bindFixture(t, "PULL-004");
  await app.nav.reset({ page: 'pullToRefresh' });
  const view = (await app.page({ name: 'pullToRefresh' }, { timeout: 30_000 })).view;
  await view.testId('pull-refresh-page').waitFor({ timeout: 30_000 });
  const lastRow = view.testId('pull-refresh-row').last();
  await expect(lastRow).not.toBeInViewport();
  // A slow swipe of the same finger travel stops well short of the end.
  await view.swipe({ from: await at(view, 0.5, 0.68), to: await at(view, 0.5, 0.26), durationMs: 400 });
  await expect(lastRow).not.toBeInViewport();
  await view.fling('up', 4000);
  await expect(lastRow).toBeInViewport({ timeout: 3_000 });
});
