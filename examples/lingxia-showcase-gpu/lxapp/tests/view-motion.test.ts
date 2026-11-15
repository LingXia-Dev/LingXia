import { expect, spec } from '@lingxia/test';
import { openDemo } from './open-demo';

// The panel's box takes its new height at once; what is drawn grows to it.
spec('a panel opens to its full height, the row after it moves down, and it closes again', { covers: ['view:transition-size'], start: openDemo('panel') }, async (t) => {
  const view = t.app.view;
  const state = view.testId('lab-panel-state');
  const box = view.testId('lab-panel-box');
  const after = view.testId('lab-panel-after');
  await expect(state).toHaveAttribute('data-state', 'open=false');
  const closed = await box.boundingBox();
  const before = await after.boundingBox();
  expect(Math.round(closed!.height)).toBe(0);
  await view.testId('lab-panel-toggle').click();
  await expect(state).toHaveAttribute('data-state', 'open=true');
  const opened = await box.boundingBox();
  expect(Math.round(opened!.height)).toBe(96);
  expect(Math.round((await after.boundingBox())!.y - before!.y)).toBe(96);
  await view.testId('lab-panel-toggle').click();
  await expect(state).toHaveAttribute('data-state', 'open=false');
  expect(Math.round((await box.boundingBox())!.height)).toBe(0);
});
