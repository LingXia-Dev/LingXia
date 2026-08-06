import { expect } from '@rongjs/test';
import type { LxAppDriver } from 'lingxia-types/automation';
import { waitForCurrentPage } from '../helpers/page.js';
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
