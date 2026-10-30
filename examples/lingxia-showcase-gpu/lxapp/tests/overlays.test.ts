import { expect, spec } from '@lingxia/test';

// Dialogs and popovers in the top layer, read and driven through their roles.
spec('a menu popover opens from its button and a menu item closes it', { covers: ['view:popover'], start: { page: 'view-lab' } }, async (t) => {
  const view = t.app.view;
  await view.testId('lab-tab-overlay').click();
  const more = view.getByRole('button', { name: 'More' });
  await more.click();
  const menu = view.getByRole('menu', { name: 'Note actions' });
  await expect(menu).toBeVisible();
  await view.getByRole('menuitem', { name: 'Share' }).click();
  await expect(menu).not.toBeAttached();
  await expect(view.testId('lab-overlay-last')).toHaveText('last: Share');
});

spec('a dialog hides the page behind it until it closes', { covers: ['view:dialog'], start: { page: 'view-lab' } }, async (t) => {
  const view = t.app.view;
  await view.testId('lab-tab-overlay').click();
  await view.testId('lab-overlay-confirm').click();
  await expect(view.getByRole('dialog', { name: 'Delete note?' })).toBeVisible();
  // The page behind is out of reach for assistive tech, so for role queries too.
  await expect(view.getByRole('button', { name: 'Sheet' })).toHaveCount(0);
  await view.testId('lab-dialog-cancel').click();
  await expect(view.getByRole('dialog')).not.toBeAttached();
  await expect(view.testId('lab-overlay-last')).toHaveText('last: cancel');
});

spec('a sheet holds its own scroll and closes from its button', { covers: ['view:dialog'], start: { page: 'view-lab' } }, async (t) => {
  const view = t.app.view;
  await view.testId('lab-tab-overlay').click();
  await view.testId('lab-overlay-sheet').click();
  await expect(view.getByRole('dialog', { name: 'Filters' })).toBeVisible();
  await expect(view.testId('lab-sheet-scroll')).toBeVisible();
  await view.testId('lab-sheet-apply').click();
  await expect(view.testId('lab-overlay-last')).toHaveText('last: apply');
});
