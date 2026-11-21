import { expect, test } from '@lingxia/test';
import { openDemo } from './open-demo';

test('select below the fold picks options and refuses a disabled one', { covers: ['view:select'], start: openDemo('select') }, async (t) => {
  const view = t.app.view;
  const plan = view.testId('lab-input-plan');
  const values = view.testId('lab-select-values');
  await plan.selectOption('yearly');
  await expect(values).toHaveAttribute('data-state', /plan=yearly /);
  const started = Date.now();
  let refused = '';
  try { await plan.selectOption('quarterly'); } catch (error) { refused = String(error); }
  const took = Date.now() - started;
  console.log(`disabled option: ${took} ms: ${refused}`);
  expect(refused).not.toBe('');
  expect(took).toBeLessThan(5000);
  await expect(values).toHaveAttribute('data-state', /plan=yearly /);
  await plan.selectOption('monthly');
  await expect(values).toHaveAttribute('data-state', /plan=monthly /);
});
