import { expect, spec } from '@lingxia/test';
import { openDemo } from './open-demo';

// Touch select sheet: the check marks a choice, never keyboard focus; Done commits the tapped one.
spec('a select sheet with no value ticks nothing until a tap, and Done commits that tap', {
  covers: ['view:select', 'view:dialog'], start: openDemo('select'), requires: { pointer: 'coarse' },
}, async (t) => {
  const view = t.app.view;
  const values = view.testId('lab-select-values');
  // `plan` starts empty on a fresh page; a value left by an earlier spec must be the only tick.
  const current = /plan=(\w*)/.exec((await values.textContent()) ?? '')?.[1] ?? '';
  console.log(`plan before: "${current}"`);
  await view.testId('lab-input-plan').click();
  await expect(view.testId('lx-picker')).toBeVisible();
  for (const [index, value] of [[0, 'monthly'], [2, 'yearly']] as const) {
    const option = view.testId(`lx-picker-option-${index}`);
    if (value === current) await expect(option).toBeSelected(); else await expect(option).not.toBeSelected();
  }
  if (current === 'yearly') t.skip('needs a page where plan is not already yearly');
  await view.testId('lx-picker-option-2').click();
  await expect(view.testId('lx-picker-option-2')).toBeSelected();
  await expect(view.testId('lx-picker-option-0')).not.toBeSelected();
  await view.testId('lx-picker-done').click();
  await expect(view.testId('lx-picker')).not.toBeAttached();
  await expect(values).toHaveAttribute('data-state', /plan=yearly /);
});

// A choice made through automation while the sheet is open closes the sheet, as a tap and Done would.
spec('selectOption with the select sheet open picks the option and closes the sheet', {
  covers: ['view:select', 'view:dialog'], start: openDemo('select'), requires: { pointer: 'coarse' },
}, async (t) => {
  const view = t.app.view;
  const plan = view.testId('lab-input-plan');
  await plan.click();
  await expect(view.testId('lx-picker')).toBeVisible();
  await plan.selectOption('monthly');
  await expect(view.testId('lx-picker')).not.toBeAttached();
  await expect(view.testId('lab-select-values')).toHaveAttribute('data-state', /plan=monthly /);
});
