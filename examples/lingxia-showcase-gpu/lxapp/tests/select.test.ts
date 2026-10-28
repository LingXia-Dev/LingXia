import { expect, spec } from '@lingxia/test';

spec('select below the fold picks options and refuses a disabled one', { start: { page: 'view-lab' } }, async (t) => {
  const view = t.app.view;
  await view.testId('lab-tab-input').click();
  const plan = view.testId('lab-input-plan');
  const values = view.testId('lab-input-values');
  await plan.selectOption('yearly');
  await expect(values).toContainText('plan=yearly');
  const started = Date.now();
  let refused = '';
  try { await plan.selectOption('quarterly'); } catch (error) { refused = String(error); }
  const took = Date.now() - started;
  console.log(`disabled option: ${took} ms: ${refused}`);
  expect(refused).not.toBe('');
  expect(took).toBeLessThan(5000);
  await expect(values).toContainText('plan=yearly');
  await plan.selectOption('monthly');
  await expect(values).toContainText('plan=monthly');
});
