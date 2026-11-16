import { expect, spec } from '@lingxia/test';
import { openDemo } from './open-demo';

// The panel's box takes its new height at once; what is drawn grows to it.
spec('a panel opens to its full height, the row after it moves down, and it closes again', { covers: ['view:transition-size'], start: openDemo('panel') }, async (t) => {
  const view = t.app.view;
  const state = view.testId('lab-panel-state');
  const box = view.testId('lab-panel-box');
  const after = view.testId('lab-panel-after');
  await expect(state).toHaveAttribute('data-state', 'open=false');
  // Closed, the box has no height to measure: the row after it says where it ends. Measured from
  // the heading, since the click may scroll the page to bring it on screen.
  const toggle = view.testId('lab-panel-toggle');
  const gap = async () => Math.round((await after.boundingBox()).y - (await toggle.boundingBox()).y);
  const before = await gap();
  await toggle.click();
  await expect(state).toHaveAttribute('data-state', 'open=true');
  expect(Math.round((await box.boundingBox()).height)).toBe(96);
  expect(await gap() - before).toBe(96);
  await toggle.click();
  await expect(state).toHaveAttribute('data-state', 'open=false');
  expect(await gap() - before).toBe(0);
});
