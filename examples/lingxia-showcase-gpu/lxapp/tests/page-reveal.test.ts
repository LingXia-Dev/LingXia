import { expect, test } from '@lingxia/test';
import { runtimePlatform } from '@lingxia/test/showcase/helpers/platform.js';

// A page pushed again loads a new instance onto the surface that still holds the old one's
// frame; that frame is not the new page's, so the reveal must wait for a fresh present.
test('a page pushed again is not revealed on its previous frame', {
  id: 'CHROME-REPUSH-001', covers: ['lx.nav.go', 'lx.nav.back'],
  start: { page: 'view' },
}, async (t) => {
  const app = t.app;
  const platform = await runtimePlatform(app);
  // Without a host-side reveal the host shows the page itself; its first present still says
  // whether it showed a frame already on the surface.
  const hostReveal = ['harmony', 'android'].includes(platform);
  let reveals = -1;
  for (let round = 0; round < 3; round++) {
    await app.nav.to({ page: 'motion-target' });
    // Reveal statistics outlive a reload: wait for this push's own reveal.
    const shown = await t.waitFor(() => app.metrics({ page: 'motion-target' }), {
      until: m => m.firstPresentAt !== null && m.reveal.count > reveals
        && (hostReveal ? m.reveal.lastReveal !== null : m.firstPresentReason !== null),
    });
    reveals = shown.reveal.count;
    await t.attach(`push-${round}`, {
      reveal: shown.reveal, firstPresentAt: shown.firstPresentAt, firstPresentReason: shown.firstPresentReason,
    });
    if (!hostReveal) {
      if (round > 0) expect(shown.firstPresentReason, `push ${round + 1}`).not.toBe('current');
    } else {
      const { reason, atMs } = shown.reveal.lastReveal!;
      if (round > 0) expect(reason, `push ${round + 1}`).not.toBe('current');
      // Revealed on a frame means on this instance's first one, never before it.
      if (reason === 'frame') expect(atMs, `push ${round + 1}`).toBeGreaterThanOrEqual(shown.firstPresentAt!);
    }
    await app.nav.back();
    await expect.poll(async () => (await app.nav.current()).name).toBe('view');
  }
});
