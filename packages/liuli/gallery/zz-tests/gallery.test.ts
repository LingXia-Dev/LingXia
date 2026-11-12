import { expect, spec } from '@lingxia/test';

// Liuli's gallery on a device: every page opens, and each control answers taps and keys as its
// accessibility pattern says. Controls report their state as `data-state`, which locators read.

const pause = (ms: number) => new Promise(resolve => setTimeout(resolve, ms));

// Every tile on the home page, in its order; core's gallery.test.mjs keeps this equal to the manifest.
const pages = [
  'colour', 'typography', 'shape-and-spacing', 'elevation', 'motion',
  'button', 'tabs', 'text-input', 'select', 'checkbox-input', 'radio-group', 'switch', 'slider', 'field',
  'badge', 'progress-bar', 'dialog', 'dropdown-menu', 'tooltip', 'card', 'item', 'details', 'avatar',
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
  const before = await digest.getAttribute('data-state');
  await digest.click();
  await expect(digest).not.toHaveAttribute('data-state', before ?? '');
  await digest.press('Space');
  await expect(digest).toHaveAttribute('data-state', before ?? '');
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
  await expect(tab('overview')).toHaveAttribute('data-state', 'active');
  await tab('activity').click();
  await expect(tab('activity')).toHaveAttribute('data-state', 'active');
  await expect(panel).toContainText('Ana edited');
  await tab('activity').press('ArrowRight');
  await expect(tab('members')).toHaveAttribute('data-state', 'active');
  await expect(tab('members')).toBeFocused();
  await expect(panel).toContainText('Kai Chen');
  await tab('members').press('ArrowRight');
  await expect(tab('overview')).toHaveAttribute('data-state', 'active');
  await view.testId('range-month').click();
  await expect(view.testId('range-month')).toHaveAttribute('data-state', 'active');
  await expect(view.testId('range-panel')).toContainText('21 hours');
});

spec('radio group: a tap chooses; arrows move and choose; the required error clears', { start: { page: 'radio-group' }, timeout: 60_000 }, async (t) => {
  const view = t.app.view;
  const option = (value: string) => view.testId(`delivery-${value}`);
  await expect(option('standard')).toHaveAttribute('data-state', 'checked');
  await option('express').click();
  await expect(option('express')).toHaveAttribute('data-state', 'checked');
  await expect(option('standard')).toHaveAttribute('data-state', 'unchecked');
  await option('express').press('ArrowDown');
  await expect(option('pickup')).toHaveAttribute('data-state', 'checked');
  await expect(option('pickup')).toBeFocused();
  await view.testId('plan-team').click();
  await expect(view.testId('plan-team')).toHaveAttribute('data-state', 'checked');
  const reason = view.testId('reason');
  await expect(reason).toHaveAttribute('aria-invalid', 'true');
  await view.testId('reason-other').click();
  await expect(reason).not.toHaveAttribute('aria-invalid', 'true');
});

spec('select: a tap opens the platform list; a choice lands in the box', { start: { page: 'select' }, timeout: 60_000 }, async (t) => {
  const view = t.app.view;
  const country = view.getByRole('combobox', { name: 'Country' });
  await expect(country).toHaveText('Choose a country');
  await country.click();
  await pause(600);
  await t.attach('select-open.png', await view.screenshot());
  await country.press('Escape');
  await country.selectOption('fr');
  await expect(country).toHaveText('France');
  const role = view.getByRole('combobox', { name: 'Role' });
  await expect(role).toHaveAttribute('aria-invalid', 'true');
  await role.selectOption('editor');
  await expect(role).not.toHaveAttribute('aria-invalid', 'true');
});

spec('switch: a tap flips it; Space flips it back', { start: { page: 'switch' }, timeout: 60_000 }, async (t) => {
  const sync = t.app.view.getByRole('switch', { name: 'Sync over mobile data' });
  await expect(sync).toHaveAttribute('data-state', 'on');
  await sync.click();
  await expect(sync).toHaveAttribute('data-state', 'off');
  await sync.press('Space');
  await expect(sync).toHaveAttribute('data-state', 'on');
});

spec('details: a row opens and closes; an accordion keeps one open', { start: { page: 'details' }, timeout: 60_000 }, async (t) => {
  const view = t.app.view;
  const summary = (id: string) => view.testId(`${id}-summary`);
  await summary('active').click();
  await expect(summary('active')).toHaveAttribute('data-state', 'open');
  await expect(view.testId('active-body')).toBeVisible();
  await summary('active').press('Enter');
  await expect(view.testId('active-body')).toHaveCount(0);
  await expect(summary('refunds')).toHaveAttribute('data-state', 'open');
  await summary('export').click();
  await expect(summary('export')).toHaveAttribute('data-state', 'open');
  await expect(summary('refunds')).toHaveAttribute('data-state', 'closed');
});

spec('tooltip: a press opens the note; a tap outside closes it', { start: { page: 'tooltip' }, timeout: 60_000 }, async (t) => {
  const view = t.app.view;
  const button = view.testId('storage');
  const note = view.testId('storage-content');
  await expect(note).toHaveCount(0);
  await button.click();
  await expect(note).toBeVisible();
  await expect(note).toContainText('Attachments count');
  await expect(button).toHaveAttribute('data-state', 'open');
  const vp = await view.viewport();
  await view.swipe({ from: [vp.width / 2, vp.height - 40], to: [vp.width / 2, vp.height - 40], durationMs: 60 });
  await expect(note).toHaveCount(0);
  await expect(button).toHaveAttribute('data-state', 'closed');
});
