import { expect, spec } from '@lingxia/test';
import { openDemo } from './open-demo';

// `useBox`: an element's laid-out size, read in render.
spec('a box reads the size its element was laid out at', { covers: ['view:layout'], start: openDemo('pointer') }, async (t) => {
  const view = t.app.view;
  const state = view.testId('lab-scrub-state');
  const text = await t.waitFor(() => state.getAttribute('data-state').then(value => value ?? ''), { until: value => /box=[\d.]+x[\d.]+/.test(value) });
  const [, width, height] = /box=([\d.]+)x([\d.]+)/.exec(text)!.map(Number);
  // The plot has no padding or border: its content box is its bounding box.
  const box = await view.testId('lab-scrub-plot').boundingBox();
  expect(Math.abs(width - box.width)).toBeLessThanOrEqual(0.5);
  expect(Math.abs(height - box.height)).toBeLessThanOrEqual(0.5);
});
