import { expect, spec } from '@lingxia/test';

// An editor that owns a list: focus and blur events, a popover anchored to the field box, keys
// taken from the editor while the caret stays in it, and a read-only editor.
spec('a field opens its suggestions against itself on focus and takes a tapped one without blurring first', { covers: ['view:input', 'view:popover'], start: { page: 'view-lab' } }, async (t) => {
  const view = t.app.view;
  await view.testId('lab-tab-engine').click();
  const input = view.testId('lab-type-input');
  const state = view.testId('lab-type-state');
  await expect(state).toHaveAttribute('data-state', 'open=false active=0 last=none');
  await input.click();
  await expect(state).toHaveAttribute('data-state', 'open=true active=0 last=focus');
  // The list sits against the field box, its start edge on the box's: under it, or over it
  // where the keyboard leaves no room below. It follows the field while the keyboard moves it.
  const list = view.testId('lab-type-list');
  await expect(list).toBeVisible();
  let gap = Infinity;
  for (let attempt = 0; attempt < 20 && gap > 1; attempt++) {
    if (attempt) await new Promise((resolve) => setTimeout(resolve, 100));
    const [field, panel] = [await view.testId('lab-type-field').boundingBox(), await list.boundingBox()];
    const under = Math.abs(panel.y - (field.y + field.height + 4));
    const over = Math.abs(panel.y + panel.height - (field.y - 4));
    gap = Math.max(Math.abs(panel.x - field.x), Math.min(under, over));
  }
  expect(gap).toBeLessThanOrEqual(1);
  await input.fill('ch');
  await expect(state).toHaveAttribute('data-state', /open=true/);
  // The tap is the option's: its click runs, and no blur closes the list under it first.
  await view.testId('lab-type-option-Cherry').click();
  await expect(state).toHaveAttribute('data-state', 'open=false active=0 last=picked Cherry');
  await expect(input).toHaveValue('Cherry');
});

spec('an editor gives the page the keys it lists and keeps the caret', { covers: ['view:input', 'view:keys'], start: { page: 'view-lab' } }, async (t) => {
  const view = t.app.view;
  await view.testId('lab-tab-engine').click();
  const input = view.testId('lab-type-input');
  const state = view.testId('lab-type-state');
  await input.click();
  await expect(state).toHaveAttribute('data-state', /open=true/);
  await input.press('ArrowDown');
  await input.press('ArrowDown');
  await input.press('ArrowUp');
  await expect(state).toHaveAttribute('data-state', /active=1/);
  await input.press('Enter');
  await expect(state).toHaveAttribute('data-state', 'open=false active=1 last=picked Apricot');
  await expect(input).toHaveValue('Apricot');
  await input.fill('g');
  await expect(state).toHaveAttribute('data-state', /open=true/);
  await input.press('Escape');
  await expect(state).toHaveAttribute('data-state', 'open=false active=0 last=escape');
});

spec('moving to another editor blurs the first, and a read-only editor keeps its text', { covers: ['view:input'], start: { page: 'view-lab' } }, async (t) => {
  const view = t.app.view;
  await view.testId('lab-tab-engine').click();
  const state = view.testId('lab-type-state');
  await view.testId('lab-type-input').click();
  await expect(state).toHaveAttribute('data-state', /last=focus/);
  const reference = view.testId('lab-type-readonly');
  await reference.click();
  await expect(state).toHaveAttribute('data-state', /last=blur/);
  // Typing at it changes nothing, however the platform's keyboard answers.
  await reference.fill('changed').catch(() => {});
  await expect(reference).toHaveValue('Read only');
});
