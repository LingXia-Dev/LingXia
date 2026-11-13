import { expect, spec } from '@lingxia/test';

const upright = async (view: { viewport(): Promise<{ width: number; height: number }> }) => {
  const area = await view.viewport();
  return area.height > area.width;
};

// A phone turns for a wide full-screen picture; a tall one keeps it upright, and leaving changes nothing.
spec('a portrait full-screen video keeps the window upright', { start: { page: 'video' }, requires: { input: ['touch'] } }, async (t) => {
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
