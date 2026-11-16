import type { Fixture } from '@lingxia/test';

/** Skips a spec that needs what a covered desktop window does not give: playing video, or frame
 *  times measured against a display (a covered window's frames tick on a timer and reach no
 *  screen). Animations run covered. Read once at the start, so a window covered mid-spec still
 *  fails. */
export async function skipWhenCovered(t: Fixture, page: string): Promise<void> {
  const metrics = await t.app.metrics({ page });
  if (metrics.reveal.windowCovered) t.skip('window is covered: video pauses and frame times are not a display\'s');
}
