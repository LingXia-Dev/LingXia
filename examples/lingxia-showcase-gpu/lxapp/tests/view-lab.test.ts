import { expect, spec } from '@lingxia/test';

// GPU-only fixtures read through role and accessible name, the way a screen reader does.
spec('view lab answers locators by role and name', { start: { page: 'view-lab' } }, async (t) => {
  const view = t.app.view;
  await view.getByRole('button', { name: 'Input' }).click();
  await expect(view.getByRole('heading', { name: 'Monthly bills' })).toBeVisible();
  await expect(view.getByRole('listitem')).toHaveCount(3);
  const status = view.getByRole('status');
  await expect(status).toHaveText('Not saved yet');
  await view.getByRole('button', { name: 'Save' }).click();
  await expect(status).toHaveText('Saved 1 times');
});

spec('view lab editors take text through their accessible names', { start: { page: 'view-lab' } }, async (t) => {
  const view = t.app.view;
  await view.getByRole('button', { name: 'Input' }).click();
  // Below the fold: the locator reveals the editor before it types.
  await view.getByRole('textbox', { name: 'Amount' }).fill('12');
  await view.getByRole('textbox', { name: 'One-time code' }).fill('123456');
  await expect(view.testId('lab-input-values')).toContainText('code=123456 amount=12');
});
