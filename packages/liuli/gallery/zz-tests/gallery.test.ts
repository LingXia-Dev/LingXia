import { expect, spec } from '@lingxia/test';

// Liuli's gallery on a device: every page opens, and each control answers taps and keys as its
// accessibility pattern says. States are asserted as a screen reader hears them: checked, selected, expanded.

const pause = (ms: number) => new Promise(resolve => setTimeout(resolve, ms));

// Every tile on the home page, in its order; core's gallery.test.mjs keeps this equal to the manifest.
const pages = [
  'colour', 'typography', 'shape-and-spacing', 'elevation', 'motion',
  'button', 'tabs', 'text-input', 'select', 'checkbox-input', 'radio-group', 'switch', 'slider', 'field',
  'badge', 'progress-bar', 'banner', 'dialog', 'dropdown-menu', 'tooltip', 'card', 'item', 'details', 'avatar',
  'sign-in', 'settings', 'uploads',
];

spec('gallery: the home page opens every page and comes back', { start: { page: 'home' }, timeout: 240_000 }, async (t) => {
  const view = t.app.view;
  for (const page of pages) {
    const tile = view.testId(`open-${page}`);
    await tile.click();
    await expect(tile).toHaveCount(0);
    await t.attach(`${page}.png`, await view.screenshot());
    await t.app.nav.back();
    await expect(view.testId(`open-${page}`)).toBeAttached();
  }
});

spec('button: an async action shows busy, then settles', { start: { page: 'button' }, timeout: 60_000 }, async (t) => {
  const publish = t.app.view.testId('publish');
  await publish.click();
  await expect(publish).toBeEnabled();
});

spec('checkbox: a tap toggles it; Space toggles it back', { start: { page: 'checkbox-input' }, timeout: 60_000 }, async (t) => {
  const digest = t.app.view.getByRole('checkbox', { name: 'Weekly digest' });
  await expect(digest).toBeChecked();
  await digest.click();
  await expect(digest).not.toBeChecked();
  await digest.press('Space');
  await expect(digest).toBeChecked();
});

spec('dialog: an alert dialog ignores a backdrop tap; a form closes on Escape', { start: { page: 'dialog' }, timeout: 90_000 }, async (t) => {
  const view = t.app.view;
  await view.getByRole('button', { name: 'Delete note' }).click();
  const alert = view.getByRole('alertdialog', { name: 'Delete this note?' });
  await expect(alert).toBeVisible();
  const vp = await view.viewport();
  await view.swipe({ from: [8, vp.height - 8], to: [8, vp.height - 8], durationMs: 60 });
  await pause(400);
  await expect(alert).toBeVisible();
  await view.getByRole('button', { name: 'Delete' }).click();
  await expect(alert).toHaveCount(0);
  await expect(view.testId('outcome')).toHaveText('Deleted');
  await view.getByRole('button', { name: 'Rename' }).click();
  const form = view.getByRole('dialog', { name: 'Rename notebook' });
  await expect(form).toBeVisible();
  await view.getByRole('textbox', { name: 'Name' }).fill('Travel');
  await view.getByRole('button', { name: 'Save' }).click();
  await expect(form).toHaveCount(0);
  await view.getByRole('button', { name: 'Rename' }).click();
  await expect(form).toBeVisible();
  await form.press('Escape');
  await expect(form).toHaveCount(0);
});

spec('dropdown: an item runs and closes the menu; a tap outside closes it', { start: { page: 'dropdown-menu' }, timeout: 90_000 }, async (t) => {
  const view = t.app.view;
  const menu = view.getByRole('menu');
  await view.getByRole('button', { name: 'Sort by' }).click();
  await expect(menu).toBeVisible();
  await view.testId('sort-updated').click();
  await expect(menu).toHaveCount(0);
  await expect(view.testId('order')).toHaveText('Last updated');
  await view.getByRole('button', { name: 'Sort by' }).click();
  await expect(menu).toBeVisible();
  const vp = await view.viewport();
  await view.swipe({ from: [vp.width / 2, vp.height - 40], to: [vp.width / 2, vp.height - 40], durationMs: 60 });
  await expect(menu).toHaveCount(0);
});

spec('dropdown: Enter opens on the first item; Down then Enter runs the second', { start: { page: 'dropdown-menu' }, timeout: 60_000 }, async (t) => {
  const view = t.app.view;
  await view.getByRole('button', { name: 'Sort by' }).press('Enter');
  await expect(view.getByRole('menuitem', { name: 'Name' })).toBeFocused();
  await view.getByRole('menuitem', { name: 'Name' }).press('ArrowDown');
  await expect(view.testId('sort-updated')).toBeFocused();
  await view.testId('sort-updated').press('Enter');
  await expect(view.getByRole('menu')).toHaveCount(0);
  await expect(view.testId('order')).toHaveText('Last updated');
});

spec('text input: an invalid email is marked; typing reaches the field', { start: { page: 'text-input' }, timeout: 60_000 }, async (t) => {
  const view = t.app.view;
  const email = view.getByRole('textbox', { name: 'Email' });
  await email.fill('abc');
  await expect(email).toHaveAttribute('aria-invalid', 'true');
  const search = view.getByRole('textbox', { name: 'Search notes' });
  await search.fill('milk');
  await expect(search).toHaveValue('milk');
});

spec('slider: arrows step it, Home and End reach the ends, a drag moves it', { start: { page: 'slider' }, timeout: 60_000 }, async (t) => {
  const view = t.app.view;
  const volume = view.getByRole('slider', { name: 'Volume' });
  await expect(volume).toHaveValue('40');
  await volume.press('ArrowRight');
  await expect(volume).toHaveValue('41');
  await volume.press('Home');
  await expect(volume).toHaveValue('0');
  await volume.press('End');
  await expect(volume).toHaveValue('100');
  await view.drag(volume, { dx: -150, dy: 0 });
  await expect(volume).not.toHaveValue('100');
});

spec('tabs: a tap chooses a tab and its panel; arrows choose on arrival', { start: { page: 'tabs' }, timeout: 60_000 }, async (t) => {
  const view = t.app.view;
  const tab = (value: string) => view.testId(`project-${value}`);
  const panel = view.testId('project-panel');
  await expect(tab('overview')).toBeSelected();
  await tab('activity').click();
  await expect(tab('activity')).toBeSelected();
  await expect(panel).toContainText('Ana edited');
  await tab('activity').press('ArrowRight');
  await expect(tab('members')).toBeSelected();
  await expect(tab('members')).toBeFocused();
  await expect(panel).toContainText('Kai Chen');
  await tab('members').press('ArrowRight');
  await expect(tab('overview')).toBeSelected();
  await view.testId('range-month').click();
  await expect(view.testId('range-month')).toBeSelected();
  await expect(view.testId('range-panel')).toContainText('21 hours');
});

spec('radio group: a tap chooses; arrows move and choose; the required error clears', { start: { page: 'radio-group' }, timeout: 60_000 }, async (t) => {
  const view = t.app.view;
  const option = (value: string) => view.testId(`delivery-${value}`);
  await expect(option('standard')).toBeChecked();
  await option('express').click();
  await expect(option('express')).toBeChecked();
  await expect(option('standard')).not.toBeChecked();
  await option('express').press('ArrowDown');
  await expect(option('pickup')).toBeChecked();
  await expect(option('pickup')).toBeFocused();
  await view.testId('plan-team').click();
  await expect(view.testId('plan-team')).toBeChecked();
  const reason = view.testId('reason');
  await expect(reason).toHaveAttribute('aria-invalid', 'true');
  await view.testId('reason-other').click();
  await expect(reason).not.toHaveAttribute('aria-invalid', 'true');
});

spec('select: a choice lands in the box; a required select clears its error', { start: { page: 'select' }, timeout: 60_000 }, async (t) => {
  const view = t.app.view;
  const country = view.getByRole('combobox', { name: 'Country' });
  await expect(country).toHaveText('Choose a country');
  await country.selectOption('fr');
  await expect(country).toHaveText('France');
  const role = view.getByRole('combobox', { name: 'Role' });
  await expect(role).toHaveAttribute('aria-invalid', 'true');
  await role.selectOption('editor');
  await expect(role).not.toHaveAttribute('aria-invalid', 'true');
});

spec('select (fine pointer): the list opens under the box; a click chooses, Escape closes without choosing', { start: { page: 'select' }, requires: { pointer: 'fine' }, timeout: 60_000 }, async (t) => {
  const view = t.app.view;
  const country = view.getByRole('combobox', { name: 'Country' });
  const list = view.getByRole('listbox', { name: 'Country' });
  await country.click();
  await expect(list).toBeVisible();
  await t.attach('select-open.png', await view.screenshot());
  await list.getByRole('option', { name: 'France' }).click();
  await expect(list).toHaveCount(0);
  await expect(country).toHaveText('France');
  await country.click();
  await expect(list).toBeVisible();
  await list.press('Escape');
  await expect(list).toHaveCount(0);
  await expect(country).toHaveText('France');
});

spec('select (touch): the list opens as a sheet; Done keeps a choice, Escape drops one', { start: { page: 'select' }, requires: { pointer: 'coarse' }, timeout: 60_000 }, async (t) => {
  const view = t.app.view;
  const country = view.getByRole('combobox', { name: 'Country' });
  const sheet = view.getByRole('dialog', { name: 'Country' });
  await country.click();
  await expect(sheet).toBeVisible();
  await t.attach('select-open.png', await view.screenshot());
  await view.getByRole('option', { name: 'France' }).click();
  await view.testId('lx-picker-done').click();
  await expect(sheet).toHaveCount(0);
  await expect(country).toHaveText('France');
  await country.click();
  await expect(sheet).toBeVisible();
  await view.getByRole('option', { name: 'Italy' }).click();
  await sheet.press('Escape');
  await expect(sheet).toHaveCount(0);
  await expect(country).toHaveText('France');
});

spec('switch: a tap flips it; Space flips it back', { start: { page: 'switch' }, timeout: 60_000 }, async (t) => {
  const sync = t.app.view.getByRole('switch', { name: 'Sync over mobile data' });
  await expect(sync).toBeChecked();
  await sync.click();
  await expect(sync).not.toBeChecked();
  await sync.press('Space');
  await expect(sync).toBeChecked();
});

spec('details: a row opens and closes; an accordion keeps one open', { start: { page: 'details' }, timeout: 60_000 }, async (t) => {
  const view = t.app.view;
  const summary = (id: string) => view.testId(`${id}-summary`);
  await summary('active').click();
  await expect(summary('active')).toBeExpanded();
  await expect(view.testId('active-body')).toBeVisible();
  await summary('active').press('Enter');
  await expect(view.testId('active-body')).toHaveCount(0);
  await expect(summary('refunds')).toBeExpanded();
  await summary('export').click();
  await expect(summary('export')).toBeExpanded();
  await expect(summary('refunds')).not.toBeExpanded();
});

spec('tooltip: a press opens the note; a tap outside closes it', { start: { page: 'tooltip' }, timeout: 60_000 }, async (t) => {
  const view = t.app.view;
  const button = view.testId('storage');
  const note = view.testId('storage-content');
  await expect(note).toHaveCount(0);
  await button.click();
  await expect(note).toBeVisible();
  await expect(note).toContainText('Attachments count');
  await expect(button).toBeExpanded();
  const vp = await view.viewport();
  await view.swipe({ from: [vp.width / 2, vp.height - 40], to: [vp.width / 2, vp.height - 40], durationMs: 60 });
  await expect(note).toHaveCount(0);
  await expect(button).not.toBeExpanded();
});

spec('banner: the action runs; dismiss removes it', { start: { page: 'banner' }, timeout: 60_000 }, async (t) => {
  const view = t.app.view;
  await view.testId('payment-action').click();
  await expect(view.testId('payment-action')).toHaveText('Card updated');
  await view.getByRole('button', { name: 'Dismiss' }).first().click();
  await expect(view.testId('payment')).toHaveCount(0);
});
