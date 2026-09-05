import type { LxAppDriver } from 'lingxia-types/automation';
import { waitForCurrentPage, waitForElementText } from '../helpers/page.js';
import { expect, spec } from '@lingxia/test';
import { bindFixture, eventually, specNamespace } from '../helpers/poll.js';
import { SHOWCASE_APP_ID } from '../helpers/app.js';

interface SurfaceLifecycleState {
  showCount: number;
  hideCount: number;
  lastLifecycle: string;
}

async function surfaceLifecycleState(app: LxAppDriver): Promise<SurfaceLifecycleState | null> {
  return app.eval({
    script: `
      const page = getCurrentPages().find((candidate) => candidate.route.includes('/surface/'));
      return page
        ? {
            showCount: page.data.showCount ?? -1,
            hideCount: page.data.hideCount ?? -1,
            lastLifecycle: page.data.lastLifecycle ?? '',
          }
        : null;
    `,
  }) as Promise<SurfaceLifecycleState | null>;
}

async function waitForSurfaceLifecycle(
  app: LxAppDriver,
  predicate: (state: SurfaceLifecycleState) => boolean,
): Promise<SurfaceLifecycleState> {
  const state = await eventually(surfaceLifecycleState.bind(null, app), (
    candidate,
  ) => candidate !== null && predicate(candidate), {
    describe: 'surface page lifecycle counters',
  });
  if (state === null) throw new Error('Surface page left the stack while waiting for lifecycle state');
  return state;
}

spec("fire onShow/onHide on the same page instance across navigation", { id: "PAGE-LIFECYCLE-001", covers: ['lx.navigateTo', 'lx.navigateBack'], app: SHOWCASE_APP_ID }, async (t) => {
  const { app, defer } = bindFixture(t, "PAGE-LIFECYCLE-001");

  defer(async () => {
    await app.nav.relaunch({ page: 'home' });
  });

  await app.nav.relaunch({ page: 'home' });
  await waitForCurrentPage(app, 'home');

  await app.nav.to({ page: 'surface' });
  const shown = await waitForSurfaceLifecycle(app, (
    { showCount, hideCount },
  ) => showCount === 1 && hideCount === 0);
  expect(shown.lastLifecycle).toContain('onShow');

  // Pushing another page hides the surface page without unloading it.
  await app.nav.to({ page: 'feedback' });
  await waitForCurrentPage(app, 'feedback');
  const hidden = await waitForSurfaceLifecycle(app, ({ hideCount }) => hideCount === 1);
  expect(hidden.lastLifecycle).toContain('onHide');
  expect(hidden.showCount).toBe(1);

  // Popping back re-shows the same instance: counters keep accumulating.
  await app.nav.back();
  await waitForCurrentPage(app, 'surface');
  const reshown = await waitForSurfaceLifecycle(app, (
    { showCount, hideCount },
  ) => showCount === 2 && hideCount === 1);
  expect(reshown.lastLifecycle).toBe('onShow (#2)');
});

interface ResetDemoState {
  instanceTag: string;
  previousInstanceTag: string;
  logicCounter: number;
  moduleCounter: number;
}

async function resetDemoState(app: LxAppDriver): Promise<ResetDemoState | null> {
  return app.eval({
    script: `
      const page = getCurrentPages().find((candidate) => candidate.route.includes('/ui/'));
      return page
        ? {
            instanceTag: page.data.instanceTag ?? '',
            previousInstanceTag: page.data.previousInstanceTag ?? '',
            logicCounter: page.data.logicCounter ?? -1,
            moduleCounter: page.data.moduleCounter ?? -1,
          }
        : null;
    `,
  }) as Promise<ResetDemoState | null>;
}

async function enterResetDemo(app: LxAppDriver): Promise<ResetDemoState> {
  await app.nav.to({ page: 'ui' });
  await waitForCurrentPage(app, 'ui');
  await app.page.waitFor({ page: 'ui', css: '[data-testid="ui-navigate-to"]' });
  const state = await eventually(resetDemoState.bind(null, app), (
    candidate,
  ) => candidate !== null
    && candidate.instanceTag !== ''
    && candidate.previousInstanceTag !== '', {
    describe: 'page reset demo to report its current and stored instance tags',
  });
  if (state === null) throw new Error('Page reset demo left the stack while loading');
  return state;
}

const waitForViewCounter = (app: LxAppDriver, expected: string) => waitForElementText(
  app,
  'ui',
  '[data-testid="lifecycle-view-counter"]',
  (text) => text.trim() === expected,
);

spec("reset logic data and the rendered document when a page is re-entered", { id: "PAGE-LIFECYCLE-002", covers: ['lx.navigateTo', 'lx.navigateBack'], app: SHOWCASE_APP_ID, timeout: 45_000 }, async (t) => {
  const { app, defer } = bindFixture(t, "PAGE-LIFECYCLE-002");

  defer(async () => {
    await app.nav.relaunch({ page: 'home' });
  });

  await app.nav.relaunch({ page: 'home' });
  await waitForCurrentPage(app, 'home');

  const first = await enterResetDemo(app);
  expect(first.logicCounter).toBe(0);

  // Dirty both layers plus the DOM, and the module-scoped counter that
  // must NOT reset.
  const moduleBase = first.moduleCounter;
  await app.page.waitFor({ page: 'ui', css: '[data-testid="lifecycle-open-popup"]', state: 'attached' });
  await app.page.eval({
    page: 'ui',
    script: `document.querySelector('[data-testid="lifecycle-open-popup"]')?.scrollIntoView({ block: 'center' })`,
  });
  await app.page.click({ page: 'ui', css: '[data-testid="lifecycle-bump-logic"]' });
  await app.page.click({ page: 'ui', css: '[data-testid="lifecycle-bump-view"]' });
  await app.page.click({ page: 'ui', css: '[data-testid="lifecycle-bump-module"]' });
  await app.page.click({ page: 'ui', css: '[data-testid="lifecycle-open-popup"]' });
  await app.page.waitFor({ page: 'ui', css: '[data-testid="lifecycle-popup"]', state: 'visible' });
  await eventually(resetDemoState.bind(null, app), (
    candidate,
  ) => candidate?.logicCounter === 1, { describe: 'logic counter to reach 1' });
  await waitForViewCounter(app, '1');

  // Leaving ends the instance; the teardown lands after the pop transition.
  await app.nav.back();
  await waitForCurrentPage(app, 'home');
  // Past the deferred-teardown delay, so this covers the timer path rather
  // than the teardown being flushed by a fast re-entry.
  await new Promise<void>((resolve) => setTimeout(() => resolve(), 1_500));

  const second = await enterResetDemo(app);
  expect(second.instanceTag).not.toBe(first.instanceTag);
  expect(second.previousInstanceTag).toBe(first.instanceTag);
  expect(second.logicCounter).toBe(0);
  // Module scope survives the instance: the fresh entry sees the bump.
  expect(second.moduleCounter).toBe(moduleBase + 1);
  await waitForViewCounter(app, '0');

  const popup = await app.page.query({
    page: 'ui',
    css: '[data-testid="lifecycle-popup"]',
  });
  expect(popup.exists).toBe(false);
});

spec("stack two live instances of one route and unwind them independently", { id: "PAGE-LIFECYCLE-003", covers: ['lx.navigateTo', 'lx.navigateBack'], app: SHOWCASE_APP_ID }, async (t) => {
  const { app, defer } = bindFixture(t, "PAGE-LIFECYCLE-003");

  defer(async () => {
    await app.nav.relaunch({ page: 'home' });
  });

  const topDemoState = async (): Promise<ResetDemoState | null> => app.eval({
    script: `
      const page = getCurrentPages().at(-1);
      return page && page.route.includes('/ui/')
        ? {
            instanceTag: page.data.instanceTag ?? '',
            previousInstanceTag: page.data.previousInstanceTag ?? '',
            logicCounter: page.data.logicCounter ?? -1,
            moduleCounter: page.data.moduleCounter ?? -1,
          }
        : null;
    `,
  }) as Promise<ResetDemoState | null>;
  const waitForTopDemo = () => eventually(topDemoState, (
    candidate,
  ) => candidate !== null && candidate.instanceTag !== '', {
    describe: 'topmost drill-down instance to report its tag',
  });

  await app.nav.relaunch({ page: 'home' });
  await waitForCurrentPage(app, 'home');
  await app.nav.to({ page: 'ui' });
  await waitForCurrentPage(app, 'ui');
  const first = await waitForTopDemo();
  if (first === null) throw new Error('first drill-down entry left the stack');

  // Distinguish the first instance before drilling deeper.
  await app.page.click({ page: 'ui', css: '[data-testid="lifecycle-bump-logic"]' });
  await eventually(topDemoState, (
    candidate,
  ) => candidate?.logicCounter === 1, { describe: 'first instance counter to reach 1' });

  // Same route again: a second, independent live instance.
  await app.nav.to({ page: 'ui' });
  const second = await eventually(topDemoState, (
    candidate,
  ) => candidate !== null && candidate.instanceTag !== ''
    && candidate.instanceTag !== first.instanceTag, {
    describe: 'second drill-down entry to report its own tag',
  });
  if (second === null) throw new Error('second drill-down entry left the stack');
  expect(second.logicCounter).toBe(0);

  const stack = await app.eval({
    script: 'return getCurrentPages().map((page) => page.route);',
  }) as string[];
  expect(stack.filter((route) => route.includes('/ui/')).length).toBe(2);

  // Unwinding lands on the first instance with its own state intact.
  await app.nav.back();
  const unwound = await eventually(topDemoState, (
    candidate,
  ) => candidate !== null && candidate.instanceTag === first.instanceTag, {
    describe: 'back to land on the first drill-down instance',
  });
  if (unwound === null) throw new Error('first instance missing after unwind');
  expect(unwound.logicCounter).toBe(1);

  await app.nav.back();
  await waitForCurrentPage(app, 'home');
});

spec('deliver exactly one onLoad and one onReady to a re-entered page', {
  id: 'PAGE-LIFECYCLE-004',
  covers: ['lx.navigateTo', 'lx.navigateBack'],
  app: SHOWCASE_APP_ID,
}, async (t) => {
  const { app, defer } = bindFixture(t, 'PAGE-LIFECYCLE-004');
  defer(async () => {
    await app.nav.relaunch({ page: 'home' });
  });

  await app.nav.relaunch({ page: 'home' });
  await waitForCurrentPage(app, 'home');

  const first = await enterResetDemo(app);
  await app.nav.back();
  await waitForCurrentPage(app, 'home');
  // Let the off-screen teardown complete: the rebuild at re-entry must
  // deliver exactly one lifecycle, never a second one of its own.
  await new Promise<void>((resolve) => setTimeout(() => resolve(), 1_500));

  const second = await enterResetDemo(app);
  expect(second.previousInstanceTag).toBe(first.instanceTag);

  // Wait until every lifecycle event of the entry has landed: `onShow` can
  // trail `onReady` by a push-animation's length, and counting early would
  // misread a late event as a missing one.
  const events = await eventually(
    async () => {
      const state = await app.eval({
        script: `
          const page = getCurrentPages().find((candidate) => candidate.route.includes('/ui/'));
          return page ? page.data.events ?? [] : null;
        `,
      }) as string[] | null;
      return state;
    },
    (candidate) => Array.isArray(candidate)
      && ['onLoad', 'onShow', 'onReady'].every((name) => candidate.some((entry) => entry.endsWith(name))),
    { describe: 'the re-entered page to report its full lifecycle' },
  ) ?? [];

  const count = (name: string) => events.filter((entry) => entry.endsWith(name)).length;
  expect(count('onLoad')).toBe(1);
  expect(count('onReady')).toBe(1);
  expect(count('onShow')).toBe(1);
});

spec("park a left page instead of re-rendering it off-screen", { id: "PAGE-LIFECYCLE-005", covers: ['lx.navigateBack'], app: SHOWCASE_APP_ID }, async (t) => {
  const { app, defer } = bindFixture(t, "PAGE-LIFECYCLE-005");

  defer(async () => {
    await app.nav.relaunch({ page: 'home' });
  });

  await app.nav.relaunch({ page: 'home' });
  await waitForCurrentPage(app, 'home');

  const first = await enterResetDemo(app);
  await app.nav.back();
  await waitForCurrentPage(app, 'home');
  // Past the teardown delay: the document must now be parked blank. Anything
  // still rendered here means page code ran off-screen — mount hooks, native
  // components, media — with nobody on the page.
  await new Promise<void>((resolve) => setTimeout(() => resolve(), 1_500));

  const parked = await app.page.query({
    page: 'ui',
    css: '[data-testid="ui-navigate-to"]',
  });
  expect(parked.exists).toBe(false);

  // The rebuild belongs to the entry: coming back renders a fresh document.
  const second = await enterResetDemo(app);
  expect(second.previousInstanceTag).toBe(first.instanceTag);
});

spec("fire onLoad once per tab instance, not on preload or later switchTab", {
  id: "PAGE-LIFECYCLE-007",
  covers: ['lx.switchTab'],
  app: SHOWCASE_APP_ID,
}, async (t) => {
  const { app, defer } = bindFixture(t, "PAGE-LIFECYCLE-007");
  defer(async () => {
    await app.nav.relaunch({ page: 'home' });
  });

  await app.nav.relaunch({ page: 'home' });
  await waitForCurrentPage(app, 'home');
  // Let preloaded tab WebViews handshake. The bug fired onLoad off that
  // handshake, so switching too early would miss it.
  await new Promise<void>((resolve) => setTimeout(() => resolve(), 2_000));

  const apiLoadCount = async (): Promise<number | null> => app.eval({
    script: `
      const page = getCurrentPages().find((candidate) => candidate.route.includes('/api/'));
      return page ? page.data.loadCount ?? -1 : null;
    `,
  }) as Promise<number | null>;

  await app.nav.switchTab({ page: 'api' });
  await waitForCurrentPage(app, 'api');
  const first = await eventually(apiLoadCount, (count) => count === 1, {
    describe: 'first switchTab onto api to deliver exactly one onLoad',
  });
  expect(first).toBe(1);

  await app.nav.switchTab({ page: 'home' });
  await waitForCurrentPage(app, 'home');
  await app.nav.switchTab({ page: 'api' });
  await waitForCurrentPage(app, 'api');
  const second = await eventually(apiLoadCount, (count) => count != null, {
    describe: 'api tab to still report its load count after a second switchTab',
  });
  expect(second).toBe(1);
});

spec("unload a pushed page dropped by switchTab", { id: "PAGE-LIFECYCLE-006", covers: ['lx.switchTab', 'lx.navigateTo'], app: SHOWCASE_APP_ID }, async (t) => {
  const { app, defer } = bindFixture(t, "PAGE-LIFECYCLE-006");

  defer(async () => {
    await app.nav.relaunch({ page: 'home' });
  });

  await app.nav.relaunch({ page: 'home' });
  await waitForCurrentPage(app, 'home');

  const first = await enterResetDemo(app);
  await app.page.click({ page: 'ui', css: '[data-testid="lifecycle-bump-logic"]' });
  await eventually(resetDemoState.bind(null, app), (
    candidate,
  ) => candidate?.logicCounter === 1, { describe: 'logic counter to reach 1' });

  // switchTab keeps the tab pages it leaves warm, but a pushed page drops off
  // the stack for good — that is a departure, and departures end instances.
  await app.nav.switchTab({ page: 'api' });
  await waitForCurrentPage(app, 'api');
  const stack = await app.eval({
    script: 'return getCurrentPages().map((page) => page.route);',
  }) as string[];
  expect(stack.some((route) => route.includes('/ui/'))).toBe(false);

  const second = await enterResetDemo(app);
  expect(second.instanceTag).not.toBe(first.instanceTag);
  expect(second.previousInstanceTag).toBe(first.instanceTag);
  expect(second.logicCounter).toBe(0);
});
