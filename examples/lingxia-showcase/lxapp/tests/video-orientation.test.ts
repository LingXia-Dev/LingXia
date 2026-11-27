import { expect, test } from '@lingxia/test';

const upright = async (view: { viewport(): Promise<{ width: number; height: number }> }) => {
  const area = await view.viewport();
  return area.height > area.width;
};

// A phone turns for a wide full-screen picture; a tall one keeps it upright, and leaving changes nothing.
test('a portrait full-screen video keeps the window upright', { start: { page: 'video' }, requires: { input: ['touch'] } }, async (t) => {
  const view = t.app.view;
  expect(await upright(view)).toBe(true);
  await view.testId('video-portrait-controls-play').click();
  await view.testId('video-portrait-controls-fullscreen').click();
  const layer = view.testId('video-portrait-controls-fullscreen-view');
  await expect(layer).toBeVisible();
  // Give a wrong turn the time a right one takes.
  await new Promise<void>(resolve => setTimeout(resolve, 1500));
  expect(await upright(view)).toBe(true);
  const area = await view.viewport();
  const box = await layer.boundingBox();
  expect(box.height).toBeGreaterThanOrEqual(area.height - 1);
  await view.testId('video-portrait-controls-full-fullscreen').click();
  await expect(layer).not.toBeAttached();
  await new Promise<void>(resolve => setTimeout(resolve, 1000));
  expect(await upright(view)).toBe(true);
});

// Full screen is the whole window: the host's nav bar and the system bars give way, and come back after.
test('a full-screen video takes the whole window and gives it back on leaving', { start: { page: 'video' }, requires: { input: ['touch'] } }, async (t) => {
  const view = t.app.view;
  const page = view.testId('video-page');
  const nav = () => t.app.chrome.elements({ scope: 'navigationBar' });
  const top = async () => [Number(await page.getAttribute('data-inset-top')), Number(await page.getAttribute('data-safe-top'))];
  const [inset, safe] = await top();
  expect((await nav()).length, 'the nav bar shows inline').toBeGreaterThan(0);
  expect(inset, 'the nav bar and status bar sit over the page top').toBeGreaterThan(safe);
  await view.testId('video-portrait-controls-play').click();
  await view.testId('video-portrait-controls-fullscreen').click();
  const layer = view.testId('video-portrait-controls-fullscreen-view');
  await expect(layer).toBeVisible();
  await t.waitFor(nav, { until: items => items.length === 0 });
  // No bar is left over the top: the status bar is hidden too (a cutout may still keep room).
  await t.waitFor(top, { until: ([i, s]) => i === s && s < safe });
  const area = await t.app.chrome.viewport();
  const box = await layer.boundingBox();
  expect(box.height).toBeGreaterThanOrEqual(area.height - 1);
  await view.testId('video-portrait-controls-full-fullscreen').click();
  await expect(layer).not.toBeAttached();
  await t.waitFor(nav, { until: items => items.length > 0 });
  await t.waitFor(top, { until: ([i, s]) => i === inset && s === safe });
});
