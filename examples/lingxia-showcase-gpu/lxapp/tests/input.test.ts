import { expect, test } from '@lingxia/test';
import { openDemo } from './open-demo';

test('editors take text through their accessible names', { covers: ['view:input'], start: openDemo('fields') }, async (t) => {
  const view = t.app.view;
  // Below the fold: the locator reveals the editor before it types.
  await view.getByRole('textbox', { name: 'Amount' }).fill('12');
  await view.getByRole('textbox', { name: 'One-time code' }).fill('123456');
  await expect(view.testId('lab-input-values')).toHaveAttribute('data-state', /^code=123456 amount=12 /);
});

test('a field states required and invalid, names its hint and error, in a group with a tooltip and an alertdialog', { covers: ['view:a11y', 'view:input'], start: openDemo('form') }, async (t) => {
  const view = t.app.view;
  await expect(view.getByRole('group', { name: 'Delivery' })).toHaveCount(1);
  await expect(view.getByRole('tooltip')).toHaveText('We deliver within the city');
  const zip = view.getByRole('textbox', { name: 'ZIP code' });
  await expect(zip).toHaveAttribute('aria-required', 'true');
  await expect(zip).toHaveAttribute('aria-invalid', 'false');
  const hint = (await zip.getAttribute('aria-describedby'))!;
  expect(hint.split(' ')).toHaveLength(1);
  await zip.fill('12');
  await expect(zip).toHaveAttribute('aria-invalid', 'true');
  await expect(view.testId('lab-form-error')).toHaveText('Not a ZIP code');
  const both = (await zip.getAttribute('aria-describedby'))!.split(' ');
  expect(both).toHaveLength(2);
  expect(both[0]).toBe(hint);
  await zip.fill('12345');
  await expect(zip).toHaveAttribute('aria-invalid', 'false');
  await view.getByRole('button', { name: 'Delete address' }).click();
  const confirm = view.getByRole('alertdialog', { name: 'Delete this address?' });
  await expect(confirm).toBeVisible();
  await view.getByRole('button', { name: 'Keep it' }).click();
  await expect(confirm).toHaveCount(0);
});

test('three number fields ask for their own keyboards and take what is typed', { covers: ['view:input'], start: openDemo('keyboards') }, async (t) => {
  const view = t.app.view;
  // The automation tree does not say which keyboard a field asks for yet; the names and values do.
  await view.getByRole('textbox', { name: 'Number type' }).fill('2.5');
  await view.getByRole('textbox', { name: 'Numeric mode' }).fill('42');
  await view.getByRole('textbox', { name: 'Decimal mode' }).fill('0.75');
  await expect(view.testId('lab-key-values')).toHaveAttribute('data-state', 'number=2.5 numeric=42 decimal=0.75');
});
