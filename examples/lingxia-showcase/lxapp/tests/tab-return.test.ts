import { expect, test } from '@lingxia/test';
import { runtimePlatform } from '@lingxia/test/showcase/helpers/platform.js';

// A kept tab page shown again shows its last frame, then a new one: never black or the bare
// canvas. Where its surface may hold nothing (Apple's layer out of the window), the host covers
// it with a snapshot that a frame at the current size takes away; Android and Harmony keep the
// surface hidden until it holds this page's frame; Windows keeps the page clipped until it
// presents. Read from the presentation counters, so no camera is needed.
test('a tab returned to shows its last frame until a new one is shown', {
  id: 'CHROME-TAB-RETURN-001', covers: ['lx.nav.go'],
  start: { page: 'home' },
}, async (t) => {
  const app = t.app;
  const platform = await runtimePlatform(app);
  const hostReveal = ['harmony', 'android'].includes(platform);
  await expect(app.view.testId('home-page')).toBeVisible();
  for (const away of [0, 2000]) {
    const before = await app.metrics({ page: 'home' });
    await app.nav.go({ tab: 'api' });
    await expect(app.view.testId('api-page')).toBeVisible();
    await new Promise<void>(resolve => setTimeout(resolve, away));
    await app.nav.go({ tab: 'home' });
    const back = await t.waitFor(() => app.metrics({ page: 'home' }), {
      until: m => m.reveal.count > before.reveal.count && !m.reveal.snapshotShown
        && (m.lastPresentAt ?? 0) > (before.lastPresentAt ?? 0),
    });
    await t.attach(`away-${away}`, { platform, before: before.reveal, back: back.reveal, lastPresentAt: back.lastPresentAt });
    const label = `away ${away} ms`;
    if (back.reveal.covers > before.reveal.covers) {
      // Taken away by a frame shown since the return, at the page's current size.
      expect(back.reveal.lastCoverMs, label).not.toBeNull();
    } else if (hostReveal) {
      expect(['frame', 'current', 'deadline'], label).toContain(back.reveal.lastReveal?.reason);
    } else {
      expect(platform, `${label}: a host without its own reveal covers a return`).toBe('windows');
    }
  }
});
