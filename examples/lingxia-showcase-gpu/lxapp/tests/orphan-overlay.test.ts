import { expect, spec } from '@lingxia/test';

// An overlay goes with the control that opened it.
spec('a date picker closes when its field leaves the page', { covers: ['view:picker', 'view:dialog'], start: { page: 'spec-orphan' } }, async (t) => {
  const view = t.app.view;
  await view.testId('lab-orphan-date').click();
  await expect(view.testId('lx-picker')).toBeVisible();
  // The picker covers the page on a phone: the field goes as a pane switch would take it.
  await view.testId('lab-orphan-toggle').click();
  await expect(view.testId('lab-orphan-gone')).toBeVisible();
  await expect(view.testId('lx-picker')).not.toBeAttached();
  await view.testId('lab-orphan-toggle').click();
  await expect(view.testId('lab-orphan-date')).toBeVisible();
  await expect(view.testId('lx-picker')).not.toBeAttached();
});
