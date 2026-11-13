import { expect, spec } from '@lingxia/test';

// Dialogs and popovers in the top layer, read and driven through their roles.
spec('a menu popover opens from its button and a menu item closes it', { covers: ['view:popover'], start: { page: 'view-lab' } }, async (t) => {
  const view = t.app.view;
  await view.testId('lab-tab-feedback').click();
  // Progress's own More button shares the tab: the menu button by its id.
  const more = view.testId('lab-overlay-more');
  await more.click();
  const menu = view.getByRole('menu', { name: 'Note actions' });
  await expect(menu).toBeVisible();
  await view.getByRole('menuitem', { name: 'Share' }).click();
  await expect(menu).not.toBeAttached();
  await expect(view.testId('lab-overlay-last')).toHaveText('last: Share');
});

spec('a dialog hides the page behind it until it closes', { covers: ['view:dialog'], start: { page: 'view-lab' } }, async (t) => {
  const view = t.app.view;
  await view.testId('lab-tab-feedback').click();
  await view.testId('lab-overlay-confirm').click();
  await expect(view.getByRole('dialog', { name: 'Delete note?' })).toBeVisible();
  // The page behind is out of reach for assistive tech, so for role queries too.
  await expect(view.getByRole('button', { name: 'Sheet' })).toHaveCount(0);
  await view.testId('lab-dialog-cancel').click();
  await expect(view.getByRole('dialog')).not.toBeAttached();
  await expect(view.testId('lab-overlay-last')).toHaveText('last: cancel');
});

spec('role queries scoped to a dialog find only its own controls', { covers: ['view:dialog'], start: { page: 'view-lab' } }, async (t) => {
  const view = t.app.view;
  await view.testId('lab-tab-feedback').click();
  await view.testId('lab-overlay-confirm').click();
  const dialog = view.testId('lab-dialog');
  await expect(dialog.getByRole('heading')).toHaveText('Delete note?');
  await expect(dialog.getByRole('button')).toHaveCount(2);
  await expect(view.testId('lab-overlay-more').getByRole('button')).toHaveCount(0);
  await dialog.getByRole('button', { name: 'Cancel' }).click();
  await expect(view.getByRole('dialog')).not.toBeAttached();
  await expect(view.testId('lab-overlay-last')).toHaveText('last: cancel');
});

spec('a sheet holds its own scroll and closes from its button', { covers: ['view:dialog'], start: { page: 'view-lab' } }, async (t) => {
  const view = t.app.view;
  await view.testId('lab-tab-feedback').click();
  await view.testId('lab-overlay-sheet').click();
  await expect(view.getByRole('dialog', { name: 'Filters' })).toBeVisible();
  await expect(view.testId('lab-sheet-scroll')).toBeVisible();
  await view.testId('lab-sheet-apply').click();
  await expect(view.testId('lab-overlay-last')).toHaveText('last: apply');
});

spec('a menu opened from the keyboard focuses its first item and gives focus back when it closes', { covers: ['view:popover', 'view:a11y'], start: { page: 'view-lab' } }, async (t) => {
  const view = t.app.view;
  await view.testId('lab-tab-feedback').click();
  const more = view.testId('lab-overlay-more');
  await more.press('Enter');
  const rename = view.getByRole('menuitem', { name: 'Rename' });
  await expect(rename).toBeFocused();
  await rename.press('Enter');
  await expect(view.testId('lab-overlay-last')).toHaveText('last: Rename');
  await expect(more).toBeFocused();
});

spec('a menu moves its focus with arrows, Home and End and jumps by a typed letter', { covers: ['view:popover', 'view:a11y'], start: { page: 'view-lab' } }, async (t) => {
  const view = t.app.view;
  await view.testId('lab-tab-feedback').click();
  await view.testId('lab-overlay-more').click();
  const item = (name: string) => view.getByRole('menuitem', { name });
  await item('Rename').press('End');
  await expect(item('Delete')).toBeFocused();
  await item('Delete').press('ArrowDown');
  await expect(item('Rename')).toBeFocused();
  await item('Rename').press('s');
  await expect(item('Share')).toBeFocused();
  await item('Share').press('Home');
  await expect(item('Rename')).toBeFocused();
  await item('Rename').press('Enter');
  await expect(view.testId('lab-overlay-last')).toHaveText('last: Rename');
});

spec('Escape closes a light-dismiss popover from its trigger or its content', { covers: ['view:popover', 'view:a11y'], start: { page: 'view-lab' } }, async (t) => {
  const view = t.app.view;
  await view.testId('lab-tab-layout').click();
  const more = view.testId('lab-choice-alpha-more');
  const menu = view.testId('lab-choices-menu');
  await more.click();
  await expect(menu).toBeVisible();
  // A key goes to the focused element and the top layer, as a keyboard's does, while the popover is open.
  await more.press('Escape');
  await expect(menu).not.toBeAttached();
  await more.click();
  await expect(menu).toBeVisible();
  await menu.press('Escape');
  await expect(menu).not.toBeAttached();
});
