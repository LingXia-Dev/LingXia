import { expect, spec } from '@lingxia/test';
import type { PageTransition } from '@lingxia/types/automation';
import { runtimePlatform } from '@lingxia/test/showcase/helpers/platform.js';

// The slide is ANIMATION_DURATION_STANDARD (300 ms); host scheduling adds a little on either side.
const SLIDE_MS = [250, 600] as const;
// A 300 ms slide spans 18 display frames at 60 Hz; fewer than half moving is a visible stutter.
const MIN_MOVING_FRAMES = 9;

const slid = (slide: PageTransition, kind: PageTransition['kind']) => {
  expect(slide.kind).toBe(kind);
  expect(slide.durationMs!).toBeGreaterThanOrEqual(SLIDE_MS[0]);
  expect(slide.durationMs!).toBeLessThanOrEqual(SLIDE_MS[1]);
  expect(slide.movingFrames).toBeGreaterThanOrEqual(MIN_MOVING_FRAMES);
  // The page starts moving on the slide's first frames, not after a stall.
  expect(slide.firstMoveMs!).toBeLessThanOrEqual(100);
};

spec('a push and a pop slide the GPU page on every display frame', {
  id: 'CHROME-PUSH-POP-001', covers: ['lx.navigateTo', 'lx.navigateBack'],
  start: { page: 'view-lab' },
}, async (t) => {
  const app = t.app;
  const platform = await runtimePlatform(app);
  if (!['harmony'].includes(platform)) t.skip(`${platform} does not report page slides`);

  await app.nav.to({ page: 'motion-lab' });
  const pushed = await t.waitFor(() => app.metrics({ page: 'motion-lab' }), { until: m => m.transition?.durationMs != null });
  await t.attach('push', pushed.transition);
  slid(pushed.transition!, 'push');

  const before = (await app.metrics({ page: 'view-lab' })).transition?.atMs ?? -1;
  await app.nav.back();
  const popped = await t.waitFor(() => app.metrics({ page: 'view-lab' }), {
    until: m => m.transition?.kind === 'pop' && m.transition.atMs !== before && m.transition.durationMs != null,
  });
  await t.attach('pop', popped.transition);
  slid(popped.transition!, 'pop');
});
