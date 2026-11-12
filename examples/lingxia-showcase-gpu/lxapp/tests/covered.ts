import type { Fixture } from '@lingxia/test';

/** Skips a spec that needs frames on screen or playing video while other windows cover the app's:
 *  a covered desktop window parks animations and video by design. Read once at the start, so a
 *  window covered mid-spec still fails. */
export async function skipWhenCovered(t: Fixture, page: string): Promise<void> {
  const metrics = await t.app.metrics({ page });
  if (metrics.reveal.windowCovered) t.skip('window is covered: animations and video pause by design');
}
