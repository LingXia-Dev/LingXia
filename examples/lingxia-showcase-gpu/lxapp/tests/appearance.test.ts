import { expect, spec } from '@lingxia/test';
import { openDemo } from './open-demo';

const pause = (ms: number) => new Promise<void>((resolve) => setTimeout(resolve, ms));

// A theme change repaints without running a View that never read the appearance; one that reads
// it, even from a later render on, runs again and shows the new theme.
spec('a page that starts reading the theme runs again on the next theme change', { covers: ['view:environment'], start: openDemo('theme') }, async (t) => {
  const view = t.app.view;
  const state = view.testId('lab-appearance-runs');
  const runs = async () => Number(/runs=(\d+)/.exec((await state.getAttribute('data-state')) ?? '')?.[1]);
  await expect(state).toHaveAttribute('data-state', /theme=unread$/);
  try {
    await view.environment({ theme: 'light' });
    await pause(400);
    await view.testId('lab-appearance-toggle').click();
    await expect(state).toHaveAttribute('data-state', /theme=light$/);
    const reading = await runs();
    await view.environment({ theme: 'dark' });
    await expect(state).toHaveAttribute('data-state', /theme=dark$/);
    expect(await runs()).toBeGreaterThan(reading);
    await view.environment({ theme: 'light' });
    await expect(state).toHaveAttribute('data-state', /theme=light$/);
  } finally {
    await view.environment(null);
  }
});
