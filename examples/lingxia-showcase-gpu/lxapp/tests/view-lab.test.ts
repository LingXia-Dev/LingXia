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

spec('view lab editors take text through their accessible names', { covers: ['view:input'], start: { page: 'view-lab' } }, async (t) => {
  const view = t.app.view;
  await view.getByRole('button', { name: 'Input' }).click();
  // Below the fold: the locator reveals the editor before it types.
  await view.getByRole('textbox', { name: 'Amount' }).fill('12');
  await view.getByRole('textbox', { name: 'One-time code' }).fill('123456');
  await expect(view.testId('lab-input-values')).toContainText('code=123456 amount=12');
});

spec('a tap on a nested switch, a row button and a rich-text link reaches only that control', { covers: ['view:switch', 'view:link', 'view:button'], start: { page: 'view-lab' } }, async (t) => {
  const view = t.app.view;
  await view.testId('lab-tab-input').click();
  const state = view.testId('lab-nested-state');
  await expect(state).toHaveText('opened=0 wifi=true removed=0');
  await view.testId('lab-nested-switch').click();
  await expect(state).toHaveText('opened=0 wifi=false removed=0');
  await view.testId('lab-nested-remove').click();
  await expect(state).toHaveText('opened=0 wifi=false removed=1');

  await view.testId('lab-tab-text').click();
  await view.testId('lab-rich-link').click();
  await expect(view.testId('lab-rich-presses')).toHaveText('Link pressed 1 times');
});

spec('the controls take a tap, a drag, typing and a fling as a user gives them', { covers: ['view:radio', 'view:slider', 'view:textarea', 'view:search', 'view:list'], start: { page: 'view-lab' } }, async (t) => {
  const view = t.app.view;
  await view.testId('lab-tab-input').click();
  const values = view.testId('lab-controls-values');
  await expect(values).toContainText('size=m volume=40 notes=0 query= picked=');

  await view.testId('lab-controls-size-l').click();
  await expect(values).toContainText('size=l');
  await expect(view.testId('lab-controls-size-xl')).not.toBeEnabled();

  // Filling the notes brings the slider above them on screen for the drag.
  await view.testId('lab-controls-notes').fill('two\nlines');
  await expect(values).toContainText('notes=9');
  await view.drag(view.testId('lab-controls-volume'), { dx: 200, dy: 0, durationMs: 400 });
  await expect.poll(async () => Number((await values.textContent()).match(/volume=(\d+)/)?.[1])).toBeGreaterThan(40);

  await view.testId('lab-controls-search').type('Row 3');
  await expect(values).toContainText('query=Row 3');
  await expect(view.testId('lab-controls-row-2')).not.toBeAttached();
  await view.testId('lab-controls-row-31').click();
  await expect(values).toContainText('picked=Row 31');

  await view.testId('lab-controls-search').fill('');
  const list = view.testId('lab-controls-list');
  await view.fling('up', 3000, { at: list });
  await expect(view.testId('lab-controls-row-40')).toBeInViewport({ timeout: 3_000 });
});
