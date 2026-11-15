import { expect, spec } from '@lingxia/test';

// `useBox`: an element's laid-out size, read in render.
spec('a box reads the size its element was laid out at', { covers: ['view:layout'], start: { page: 'view-lab' } }, async (t) => {
  const view = t.app.view;
  await view.testId('lab-tab-engine').click();
  const state = view.testId('lab-scrub-state');
  const text = await t.waitFor(() => state.textContent(), { until: value => /box=[\d.]+x[\d.]+/.test(value) });
  const [, width, height] = /box=([\d.]+)x([\d.]+)/.exec(text)!.map(Number);
  // The plot has no padding or border: its content box is its bounding box.
  const box = await view.testId('lab-scrub-plot').boundingBox();
  expect(Math.abs(width - box.width)).toBeLessThanOrEqual(0.5);
  expect(Math.abs(height - box.height)).toBeLessThanOrEqual(0.5);
});
