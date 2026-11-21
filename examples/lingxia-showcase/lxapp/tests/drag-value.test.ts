import { expect, test } from '@lingxia/test';
import { openDemo } from './open-demo';

// A drag element reads its snap point as its value whatever its role, counted from 1 as a
// slider's position is: the Places sheet's points are full (1), half (2) and off (3).
test('a sheet dialog reads its detent as its value', {
  id: 'DRAG-VALUE-SHEET-001', covers: ['view:dialog', 'view:drag'], start: openDemo('sheet'),
}, async (t) => {
  const view = t.app.view;
  await view.testId('lab-detents-open').click();
  const sheet = view.getByRole('dialog', { name: 'Places' });
  await expect(sheet).toBeVisible();
  await expect(sheet).toHaveValue('2');
  await view.drag(view.testId('lab-detents-title'), { dx: 0, dy: -260, durationMs: 700 });
  await expect(sheet).toHaveValue('1');
  await sheet.press('ArrowDown');
  await expect(sheet).toHaveValue('2');
  await sheet.performAction('dismiss');
  await expect(sheet).not.toBeAttached();
});
