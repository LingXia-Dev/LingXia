import { expect, spec } from '@lingxia/test';
import { harmonyDevice } from '@lingxia/test/showcase/helpers/harmony-device.js';

spec.configure({ requires: { args: ['harmonyDevice'] } });

// What ArkUI's accessibility layer reads, from the system's own layout dump: the field's
// description carries its hint and error, the dialog that interrupts is a Dialog.
spec('ArkUI reads a field description from aria-describedby and an alertdialog as a dialog', {
  id: 'A11Y-HARMONY-001', covers: ['view:a11y'],
  reason: 'reads the ArkUI layout through the Harmony device fixture',
  start: { page: 'view-lab' },
}, async (t) => {
  const view = t.app.view;
  const device = harmonyDevice(t);
  await view.testId('lab-tab-input').click();
  // ArkUI exposes only on-screen nodes, and each layout dump takes over a second.
  const node = async (id: string) => (await device.nodes()).find(n => n.id === id);
  const poll = (id: string, key: 'type' | 'description') =>
    expect.poll(async () => (await node(id))?.[key], { timeout: 15_000 });
  // Filling scrolls the form into view.
  const zip = view.getByRole('textbox', { name: 'ZIP code' });
  await zip.fill('12');
  await expect(zip).toHaveAttribute('aria-invalid', 'true');
  await poll('lab-form-zip', 'description').toBe('Five digits Not a ZIP code');
  await view.getByRole('button', { name: 'Delete address' }).click();
  await expect(view.getByRole('alertdialog', { name: 'Delete this address?' })).toBeVisible();
  await poll('lab-form-confirm', 'type').toBe('Dialog');
  await view.getByRole('button', { name: 'Keep it' }).click();
  // With the keyboard gone the tooltip below the field is on screen again.
  await poll('lab-form-tip', 'type').toBe('Text');
  await poll('lab-form-group', 'type').toBe('Column');
});
