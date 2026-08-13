import { expect } from '@rongjs/test';
import type { LxAppDriver } from 'lingxia-types/automation';
import { waitForCurrentPage, waitForElementText } from '../helpers/page.js';
import { contract, eventually } from '../support/contract.js';

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

contract({
  id: 'PAGE-LIFECYCLE-001',
  title: 'fire onShow/onHide on the same page instance across navigation',
  covers: ['lx.navigateTo', 'lx.navigateBack'],
  layer: 'logic',
  levels: ['semantic', 'lifecycle'],
  scope: 'portable',
  expectedOutcome: 'supported',
}, async ({ app, defer }) => {
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
}

async function resetDemoState(app: LxAppDriver): Promise<ResetDemoState | null> {
  return app.eval({
    script: `
      const page = getCurrentPages().find((candidate) => candidate.route.includes('/lifecycle/'));
      return page
        ? {
            instanceTag: page.data.instanceTag ?? '',
            previousInstanceTag: page.data.previousInstanceTag ?? '',
            logicCounter: page.data.logicCounter ?? -1,
          }
        : null;
    `,
  }) as Promise<ResetDemoState | null>;
}

async function enterResetDemo(app: LxAppDriver): Promise<ResetDemoState> {
  await app.nav.to({ page: 'lifecycle' });
  await waitForCurrentPage(app, 'lifecycle');
  await app.page.waitFor({ page: 'lifecycle', css: '[data-testid="lifecycle-page"]' });
  const state = await eventually(resetDemoState.bind(null, app), (
    candidate,
  ) => candidate !== null && candidate.instanceTag !== '', {
    describe: 'page reset demo to report its instance tag',
  });
  if (state === null) throw new Error('Page reset demo left the stack while loading');
  return state;
}

const waitForViewCounter = (app: LxAppDriver, expected: string) => waitForElementText(
  app,
  'lifecycle',
  '[data-testid="lifecycle-view-counter"]',
  (text) => text.trim() === expected,
);

contract({
  id: 'PAGE-LIFECYCLE-002',
  title: 'reset logic data and the rendered document when a page is re-entered',
  covers: ['lx.navigateTo', 'lx.navigateBack'],
  layer: 'logic',
  levels: ['semantic', 'lifecycle'],
  scope: 'portable',
  expectedOutcome: 'supported',
}, async ({ app, defer }) => {
  defer(async () => {
    await app.nav.relaunch({ page: 'home' });
  });

  await app.nav.relaunch({ page: 'home' });
  await waitForCurrentPage(app, 'home');

  const first = await enterResetDemo(app);
  expect(first.logicCounter).toBe(0);

  // Dirty both layers plus the DOM.
  await app.page.click({ page: 'lifecycle', css: '[data-testid="lifecycle-bump-logic"]' });
  await app.page.click({ page: 'lifecycle', css: '[data-testid="lifecycle-bump-view"]' });
  await app.page.click({ page: 'lifecycle', css: '[data-testid="lifecycle-open-popup"]' });
  await app.page.waitFor({ page: 'lifecycle', css: '[data-testid="lifecycle-popup"]' });
  await eventually(resetDemoState.bind(null, app), (
    candidate,
  ) => candidate?.logicCounter === 1, { describe: 'logic counter to reach 1' });
  await waitForViewCounter(app, '1');

  // Leaving ends the instance; the reset lands after the pop transition.
  await app.nav.back();
  await waitForCurrentPage(app, 'home');

  const second = await enterResetDemo(app);
  expect(second.instanceTag).not.toBe(first.instanceTag);
  expect(second.previousInstanceTag).toBe(first.instanceTag);
  expect(second.logicCounter).toBe(0);
  await waitForViewCounter(app, '0');

  const popup = await app.page.query({
    page: 'lifecycle',
    css: '[data-testid="lifecycle-popup"]',
  });
  expect(popup.exists).toBe(false);
});

contract({
  id: 'PAGE-LIFECYCLE-003',
  title: 'reject navigateTo onto a route already on the page stack',
  covers: ['lx.navigateTo'],
  layer: 'logic',
  levels: ['failure', 'lifecycle'],
  scope: 'portable',
  expectedOutcome: 'reject',
}, async ({ app, defer }) => {
  defer(async () => {
    await app.nav.relaunch({ page: 'home' });
  });

  await app.nav.relaunch({ page: 'home' });
  await waitForCurrentPage(app, 'home');
  await app.nav.to({ page: 'lifecycle' });
  await waitForCurrentPage(app, 'lifecycle');

  // One path is one page instance, so a duplicate entry would leave two stack
  // slots sharing it — popping either would end the one the other still shows.
  let rejection = '';
  try {
    await app.nav.to({ page: 'lifecycle' });
  } catch (error) {
    rejection = String(error);
  }
  expect(rejection).toContain('already on the page stack');

  const stack = await app.eval({
    script: 'return getCurrentPages().map((page) => page.route);',
  }) as string[];
  expect(stack.filter((route) => route.includes('/lifecycle/')).length).toBe(1);
});
