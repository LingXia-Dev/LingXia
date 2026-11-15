import { expect, spec } from '@lingxia/test';

const pause = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));
const runs = async (text: Promise<string>) => Number((await text).replace('runs ', ''));

// A theme change repaints without running a View that never read the appearance; one that reads
// it, even from a later render on, runs again and shows the new theme.
spec('a page that starts reading the theme runs again on the next theme change', { covers: ['view:environment'], start: { page: 'view-lab' } }, async (t) => {
  const view = t.app.view;
  await view.testId('lab-tab-engine').click();
  const count = view.testId('lab-appearance-runs');
  const theme = view.testId('lab-appearance-theme');
  await expect(theme).toHaveText('theme not read');
  try {
    await view.environment({ theme: 'light' });
    await pause(400);
    await view.testId('lab-appearance-toggle').click();
    await expect(theme).toHaveText('theme light');
    const reading = await runs(count.textContent());
    await view.environment({ theme: 'dark' });
    await expect(theme).toHaveText('theme dark');
    expect(await runs(count.textContent())).toBeGreaterThan(reading);
    await view.environment({ theme: 'light' });
    await expect(theme).toHaveText('theme light');
  } finally {
    await view.environment(null);
  }
});
