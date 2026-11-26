import { expect, test } from '@lingxia/test';
import { openDemo } from './open-demo';

const pause = (ms: number) => new Promise<void>((resolve) => setTimeout(resolve, ms));

// A theme change repaints without running a View that never read the appearance; one that reads
// it, even from a later render on, runs again and shows the new theme.
test('a page that starts reading the theme runs again on the next theme change', { covers: ['view:environment'], start: openDemo('theme') }, async (t) => {
  const view = t.app.view;
  const state = view.testId('lab-appearance-runs');
  const runs = async () => Number(/runs=(\d+)/.exec((await state.getAttribute('data-state')) ?? '')?.[1]);
  await expect(state).toHaveAttribute('data-state', /theme=unread$/);
  try {
    await view.environment({ appearance: 'light' });
    await pause(400);
    await view.testId('lab-appearance-toggle').click();
    await expect(state).toHaveAttribute('data-state', /theme=light$/);
    const reading = await runs();
    await view.environment({ appearance: 'dark' });
    await expect(state).toHaveAttribute('data-state', /theme=dark$/);
    expect(await runs()).toBeGreaterThan(reading);
    await view.environment({ appearance: 'light' });
    await expect(state).toHaveAttribute('data-state', /theme=light$/);
  } finally {
    await view.environment(null);
  }
});

// A pinned text scale is the system text size the page sees: text lays out again at it, and
// clearing the pin gives the device's own size back.
test('a pinned text scale lays the text out at that size until the pin is cleared', {
  id: 'TEXT-SCALE-001', covers: ['view:environment'], start: { page: 'view-text' },
}, async (t) => {
  const view = t.app.view;
  // A short single line: its height is its line height at any of these sizes.
  const line = view.testId('lab-select-inherited');
  const height = async () => (await line.boundingBox()).height;
  const system = await height();
  // To a tenth: line heights round to whole pixels.
  const ratio = async () => Math.round((await height()) / system * 10) / 10;
  try {
    await view.environment({ textScale: 2 });
    await expect.poll(ratio).toBe(2);
    await view.environment({ textScale: 1.3 });
    await expect.poll(ratio).toBe(1.3);
  } finally {
    await view.environment(null);
  }
  await expect.poll(ratio).toBe(1);
});
