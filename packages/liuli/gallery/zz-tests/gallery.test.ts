import { expect, test } from '@lingxia/test';
import { tap, touchMinimum, type Box } from './finger.js';

// Liuli's gallery on a device: every page opens, and each control answers taps and keys as its
// accessibility pattern says. States are asserted as a screen reader hears them: checked, selected, expanded.

const pause = (ms: number) => new Promise<void>(resolve => setTimeout(resolve, ms));

// Home's category cards and each category page's tiles, in order; core's gallery.test.mjs keeps these
// equal to the gallery.
const categories: Record<string, string[]> = {
  'category-foundations': ['colour', 'typography', 'shape-and-spacing', 'elevation', 'motion', 'playground'],
  'category-actions': ['button', 'toggle-button', 'icon-button', 'button-group', 'toggle-button-group'],
  'category-inputs': ['text-input', 'text-area', 'date-input', 'time-input', 'slider', 'field', 'search-field', 'number-input', 'calendar', 'date-range-input', 'form-layout', 'date-time-input', 'file-input', 'composer', 'list-input', 'input-group'],
  'category-selection': ['checkbox-input', 'radio-group', 'switch', 'select', 'segmented-control', 'combobox', 'checkbox-list', 'token', 'multi-select'],
  'category-navigation': ['tabs', 'nav-list', 'breadcrumb', 'pagination', 'stepper', 'link', 'page-control', 'tree-list', 'top-bar'],
  'category-feedback': ['badge', 'progress-bar', 'banner', 'empty-state', 'skeleton', 'toast', 'circular-progress', 'status-dot'],
  'category-overlays': ['dialog', 'dropdown-menu', 'tooltip', 'info-tip', 'bottom-sheet', 'action-sheet', 'popover', 'drawer', 'hover-card', 'context-menu', 'command-palette', 'tour'],
  'category-content': ['card', 'item', 'details', 'avatar', 'icon', 'table', 'markdown', 'metadata-list', 'section', 'divider', 'kbd', 'timer', 'code', 'timestamp', 'message-bubble', 'list-detail', 'split', 'qrcode', 'overflow-list'],
  'category-charts': ['stat', 'sparkline', 'bar-chart', 'line-chart', 'pie-chart', 'chart'],
  'category-media': ['video-player', 'video-controls', 'carousel', 'thumbnail', 'lightbox', 'web-view'],
  'category-documents': ['jinshu-view', 'jinshu-editor', 'jinshu-diff', 'jinshu-outline'],
  'category-patterns': ['sign-in', 'settings', 'uploads', 'list-and-detail', 'dashboard', 'conversation'],
};

test('gallery: home opens every category, and each category every page', { start: { page: 'home' }, timeout: 360_000 }, async (t) => {
  const view = t.app.view;
  for (const [category, pages] of Object.entries(categories)) {
    const card = view.testId(`open-${category}`);
    await card.click();
    await expect(card).toHaveCount(0);
    for (const page of pages) {
      const tile = view.testId(`open-${page}`);
      await tile.click();
      await expect(tile).toHaveCount(0);
      await t.attach(`${page}.png`, await view.screenshot());
      await t.app.nav.back();
      await expect(view.testId(`open-${page}`)).toBeAttached();
    }
    await t.app.nav.back();
    await expect(view.testId(`open-${category}`)).toBeAttached();
  }
});

test('gallery: home search narrows to matching components and opens one', { start: { page: 'home' }, timeout: 60_000 }, async (t) => {
  const view = t.app.view;
  await view.testId('home-search').fill('date');
  await expect(view.testId('result-date-input')).toBeAttached();
  await expect(view.testId('open-category-inputs')).toHaveCount(0);
  await view.testId('home-search').fill('zzzz');
  await expect(view.testId('home-no-results')).toBeAttached();
  await view.testId('home-search').fill('toast');
  await view.testId('result-toast').click();
  await expect(view.testId('result-toast')).toHaveCount(0);
});

test('button: an async action shows busy, then settles', { start: { page: 'button' }, timeout: 60_000 }, async (t) => {
  const publish = t.app.view.testId('publish');
  await publish.click();
  await expect(publish).toBeEnabled();
});

test('checkbox: a tap toggles it; Space toggles it back', { start: { page: 'checkbox-input' }, timeout: 60_000 }, async (t) => {
  const digest = t.app.view.getByRole('checkbox', { name: 'Weekly digest' });
  await expect(digest).toBeChecked();
  await digest.click();
  await expect(digest).not.toBeChecked();
  await digest.press('Space');
  await expect(digest).toBeChecked();
});

test('dialog: an alert dialog ignores a backdrop tap; a form closes on Escape', { start: { page: 'dialog' }, timeout: 90_000 }, async (t) => {
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

test('dropdown: an item runs and closes the menu; a tap outside closes it', { start: { page: 'dropdown-menu' }, timeout: 90_000 }, async (t) => {
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

test('dropdown: Enter opens on the first item; Down then Enter runs the second', { start: { page: 'dropdown-menu' }, timeout: 60_000 }, async (t) => {
  const view = t.app.view;
  await view.getByRole('button', { name: 'Sort by' }).press('Enter');
  await expect(view.getByRole('menuitem', { name: 'Name' })).toBeFocused();
  await view.getByRole('menuitem', { name: 'Name' }).press('ArrowDown');
  await expect(view.testId('sort-updated')).toBeFocused();
  await view.testId('sort-updated').press('Enter');
  await expect(view.getByRole('menu')).toHaveCount(0);
  await expect(view.testId('order')).toHaveText('Last updated');
});

test('dropdown: checkbox and radio items check and keep the menu open; arrows reach them', { start: { page: 'dropdown-menu' }, timeout: 60_000 }, async (t) => {
  const view = t.app.view;
  const menu = view.getByRole('menu', { name: 'View' });
  await view.testId('view-menu').click();
  await expect(menu).toBeVisible();
  await expect(view.testId('show-completed')).toBeChecked();
  await view.testId('show-archived').click();
  await expect(view.testId('show-archived')).toBeChecked();
  await expect(menu).toBeVisible();
  await view.testId('group-Week').click();
  await expect(view.testId('group-Week')).toBeChecked();
  await expect(view.testId('group-Day')).not.toBeChecked();
  await expect(menu).toBeVisible();
  await view.testId('group-Week').press('ArrowDown');
  await expect(view.testId('group-Month')).toBeFocused();
  await expect(view.testId('group-Week')).toBeChecked();
});

test('text input: an invalid email is marked; typing reaches the field', { start: { page: 'text-input' }, timeout: 60_000 }, async (t) => {
  const view = t.app.view;
  const email = view.getByRole('textbox', { name: 'Email' });
  await email.fill('abc');
  await expect(email).toHaveAttribute('aria-invalid', 'true');
  const search = view.getByRole('textbox', { name: 'Search notes' });
  await search.fill('milk');
  await expect(search).toHaveValue('milk');
});

test('slider: arrows step it, Home and End reach the ends, a drag moves it', { start: { page: 'slider' }, timeout: 60_000 }, async (t) => {
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

test('tabs: a tap chooses a tab and its panel; arrows choose on arrival', { start: { page: 'tabs' }, timeout: 60_000 }, async (t) => {
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

test('radio group: a tap chooses; arrows move and choose; the required error clears', { start: { page: 'radio-group' }, timeout: 60_000 }, async (t) => {
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

test('card choice: a tap checks a card; arrows move and choose', { start: { page: 'card' }, timeout: 60_000 }, async (t) => {
  const view = t.app.view;
  const plan = (id: string) => view.testId(`plan-${id}`);
  await expect(plan('monthly')).toBeChecked();
  await plan('yearly').click();
  await expect(plan('yearly')).toBeChecked();
  await expect(plan('monthly')).not.toBeChecked();
  await plan('yearly').press('ArrowLeft');
  await expect(plan('monthly')).toBeChecked();
  await expect(plan('monthly')).toBeFocused();
});

test('item: rows in an ItemList are read as list items of a named list', { start: { page: 'item' }, timeout: 60_000 }, async (t) => {
  const view = t.app.view;
  await expect(view.getByRole('list', { name: 'Notes' })).toBeVisible();
  await expect(view.getByRole('list', { name: 'About' })).toBeAttached();
  await expect(view.getByRole('listitem')).toHaveCount(5);
  await expect(view.getByRole('button', { name: 'Trip to the coast' })).toBeVisible();
});

test('uploads: each section is a named list, every card a list item', { start: { page: 'uploads' }, timeout: 60_000 }, async (t) => {
  const view = t.app.view;
  await expect(view.getByRole('list', { name: 'In progress' })).toBeVisible();
  await expect(view.getByRole('list', { name: 'Done today' })).toBeAttached();
  await expect(view.getByRole('listitem')).toHaveCount(4);
});

test('text area: typing reaches it, it grows, and Send submits the reply', { start: { page: 'text-area' }, timeout: 60_000 }, async (t) => {
  const view = t.app.view;
  const note = view.getByRole('textbox', { name: 'Delivery note' });
  const before = (await note.boundingBox())!.height;
  await note.fill('Blue door.\nRing twice.\nLeave it with the neighbour.\nThank you.');
  await expect(note).toHaveValue('Blue door.\nRing twice.\nLeave it with the neighbour.\nThank you.');
  expect((await note.boundingBox())!.height).toBeGreaterThan(before);
  const reply = view.getByRole('textbox', { name: 'Reply' });
  await reply.fill('On my way');
  await reply.press('Enter');
  await expect(view.testId('sent')).toHaveText('On my way');
  await expect(reply).toHaveValue('');
});

test('empty state: its title is a heading, and its action works', { start: { page: 'empty-state' }, timeout: 60_000 }, async (t) => {
  const view = t.app.view;
  await expect(view.getByRole('heading', { name: 'No notebooks yet' })).toBeVisible();
  await view.testId('create').click();
  await expect(view.getByRole('heading', { name: 'Notebook created' })).toBeVisible();
});

test('skeleton: one busy status per loading group, named by what is loading', { start: { page: 'skeleton' }, timeout: 60_000 }, async (t) => {
  const view = t.app.view;
  await expect(view.getByRole('status', { name: 'Loading notes' })).toBeVisible();
  await t.attach('skeleton.png', await view.screenshot());
});

test('select: a choice lands in the box; a required select clears its error', { start: { page: 'select' }, timeout: 60_000 }, async (t) => {
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

test('select (fine pointer): the list opens under the box; a click chooses, Escape closes without choosing', { start: { page: 'select' }, requires: { pointer: 'fine' }, timeout: 60_000 }, async (t) => {
  const view = t.app.view;
  const country = view.getByRole('combobox', { name: 'Country' });
  const list = view.getByRole('listbox', { name: 'Country' });
  await country.click();
  await expect(list).toBeVisible();
  await t.attach('select-open.png', await view.screenshot());
  await view.getByRole('option', { name: 'France' }).click();
  await expect(list).toHaveCount(0);
  await expect(country).toHaveText('France');
  await country.click();
  await expect(list).toBeVisible();
  await list.press('Escape');
  await expect(list).toHaveCount(0);
  await expect(country).toHaveText('France');
});

test('select (touch): the list opens as a sheet; Done keeps a choice, Escape drops one', { start: { page: 'select' }, requires: { pointer: 'coarse' }, timeout: 60_000 }, async (t) => {
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

test('switch: a tap flips it; Space flips it back', { start: { page: 'switch' }, timeout: 60_000 }, async (t) => {
  const sync = t.app.view.getByRole('switch', { name: 'Sync over mobile data' });
  await expect(sync).toBeChecked();
  await sync.click();
  await expect(sync).not.toBeChecked();
  await sync.press('Space');
  await expect(sync).toBeChecked();
});

test('switch: a tap on its help text flips it', { start: { page: 'switch' }, timeout: 60_000 }, async (t) => {
  const view = t.app.view;
  const sync = view.getByRole('switch', { name: 'Sync over mobile data' });
  await expect(sync).toBeChecked();
  const box = await sync.boundingBox();
  // The help line is the switch's last line, at its start; the box holds both lines.
  if (box.height < 36) throw new Error(`switch box ${box.height} tall holds only its label`);
  await view.swipe({ from: [box.x + 24, box.y + box.height - 10], to: [box.x + 24, box.y + box.height - 10], durationMs: 60 });
  await expect(sync).not.toBeChecked();
});

test('details: a row opens and closes; an accordion keeps one open', { start: { page: 'details' }, timeout: 60_000 }, async (t) => {
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

test('info tip: a press opens the note; a tap outside closes it', { start: { page: 'info-tip' }, timeout: 60_000 }, async (t) => {
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

test('banner: the action runs; dismiss removes it', { start: { page: 'banner' }, timeout: 60_000 }, async (t) => {
  const view = t.app.view;
  await view.testId('payment-action').click();
  await expect(view.testId('payment-action')).toHaveText('Card updated');
  await view.getByRole('button', { name: 'Dismiss' }).first().click();
  await expect(view.testId('payment')).toHaveCount(0);
});

test('toggle button: a tap presses it and filters the list; Space releases it', { start: { page: 'toggle-button' }, timeout: 60_000 }, async (t) => {
  const view = t.app.view;
  const unread = view.getByRole('button', { name: 'Unread' });
  await expect(unread).not.toBePressed();
  await unread.click();
  await expect(unread).toBePressed();
  await expect(view.testId('count')).toHaveText('2 notes');
  await unread.press('Space');
  await expect(unread).not.toBePressed();
  await expect(view.testId('count')).toHaveText('4 notes');
});

test('segmented control: a named radio group; a tap and an arrow choose', { start: { page: 'segmented-control' }, timeout: 60_000 }, async (t) => {
  const view = t.app.view;
  await expect(view.getByRole('radiogroup', { name: 'Units' })).toBeAttached();
  const miles = view.getByRole('radio', { name: 'Miles' });
  await miles.click();
  await expect(miles).toBeChecked();
  await expect(view.testId('distance')).toHaveText('7.7 mi');
  await miles.press('ArrowLeft');
  await expect(view.getByRole('radio', { name: 'Kilometres' })).toBeChecked();
  await expect(view.testId('distance')).toHaveText('12.4 km');
  await t.attach('segmented-control.png', await view.screenshot());
});

test('toast: Undo restores the note; tones queue and close by themselves; one with Undo waits for a swipe', { start: { page: 'toast' }, timeout: 90_000 }, async (t) => {
  const view = t.app.view;
  const toast = view.testId('toast');
  await view.testId('archive-q3-plan').click();
  await expect(toast).toBeVisible();
  await expect(view.testId('archive-q3-plan')).toHaveCount(0);
  await t.attach('toast.png', await view.screenshot());
  await view.testId('toast-action').click();
  await expect(toast).toHaveCount(0);
  await expect(view.testId('archive-q3-plan')).toBeAttached();
  await view.testId('show-success').click();
  await view.testId('show-danger').click();
  await expect(toast).toHaveCount(1);
  await expect(toast).toContainText('Changes saved');
  await toast.getByRole('button', { name: 'Dismiss' }).click();
  await expect(toast).toContainText('Could not save the note');
  await expect(toast).toHaveCount(0, { timeout: 9_000 });
  await view.testId('archive-reading-list').click();
  await expect(toast).toBeVisible();
  await pause(6_000);
  await expect(toast).toBeVisible();
  const box = await toast.boundingBox();
  await view.swipe({ from: [box.x + box.width / 2, box.y + box.height / 2], to: [box.x + box.width + 120, box.y + box.height / 2], durationMs: 120 });
  await expect(toast).toHaveCount(0);
});

test('bottom sheet: opens as a named dialog; a row closes it; a drag down dismisses it', { start: { page: 'bottom-sheet' }, timeout: 90_000 }, async (t) => {
  const view = t.app.view;
  await view.testId('move-open').click();
  const sheet = view.getByRole('dialog', { name: 'Move to' });
  await expect(sheet).toBeVisible();
  await t.attach('bottom-sheet.png', await view.screenshot());
  await view.testId('folder-Travel').click();
  await expect(sheet).toHaveCount(0);
  await expect(view.testId('folder')).toHaveText('In Travel');
  await view.testId('move-open').click();
  await expect(sheet).toBeVisible();
  const box = await sheet.boundingBox();
  await view.swipe({ from: [box.x + box.width / 2, box.y + 12], to: [box.x + box.width / 2, box.y + box.height + 200], durationMs: 200 });
  await expect(sheet).toHaveCount(0);
  await view.testId('comments-open').click();
  const comments = view.getByRole('dialog', { name: 'Comments' });
  await expect(comments).toBeVisible();
  // Halfway: its top edge sits in the lower half of the screen.
  expect((await comments.boundingBox()).y).toBeGreaterThan((await view.viewport()).height / 2);
  await t.attach('bottom-sheet-half.png', await view.screenshot());
  await view.getByRole('button', { name: 'Close' }).click();
  await expect(comments).toHaveCount(0);
});

test('popover: opens a named panel; a control inside keeps it open; Escape closes it and returns focus', { start: { page: 'popover' }, timeout: 60_000 }, async (t) => {
  const view = t.app.view;
  const trigger = view.testId('options');
  const panel = view.testId('options-panel');
  await trigger.click();
  await expect(panel).toBeVisible();
  await expect(trigger).toBeExpanded();
  await t.attach('popover.png', await view.screenshot());
  await panel.getByRole('switch', { name: 'Dates' }).click();
  await expect(view.testId('summary')).toHaveText('Previews and Dates');
  await expect(panel).toBeVisible();
  const vp = await view.viewport();
  await view.swipe({ from: [vp.width / 2, vp.height - 40], to: [vp.width / 2, vp.height - 40], durationMs: 60 });
  await expect(panel).toHaveCount(0);
  await trigger.press('Enter');
  await expect(panel).toBeVisible();
  await panel.press('Escape');
  await expect(panel).toHaveCount(0);
  await expect(trigger).toBeFocused();
});

test('date input: a tap opens the picker; a filled day reaches the page; a return before leaving is invalid', { start: { page: 'date-input' }, timeout: 60_000 }, async (t) => {
  const view = t.app.view;
  const due = view.testId('due');
  await due.click();
  await expect(view.testId('lx-picker-cancel')).toBeVisible();
  await t.attach('date-picker.png', await view.screenshot());
  await view.testId('lx-picker-cancel').click();
  await expect(view.testId('lx-picker-cancel')).toHaveCount(0);
  await due.fill('2026-12-01');
  await expect(view.testId('due-value')).toHaveText('Due 2026-12-01');
  // A committed date clamps to min, so reach the error as a person does: a valid return, then a later leave.
  const back = view.testId('back');
  await back.fill('2026-12-20');
  await expect(back).not.toHaveAttribute('aria-invalid', 'true');
  await view.testId('leave').fill('2026-12-24');
  await expect(back).toHaveAttribute('aria-invalid', 'true');
});

test('time input: a tap opens the picker; a filled time reaches the page', { start: { page: 'time-input' }, timeout: 60_000 }, async (t) => {
  const view = t.app.view;
  const remind = view.testId('remind');
  await remind.click();
  await expect(view.testId('lx-picker-cancel')).toBeVisible();
  await t.attach('time-picker.png', await view.screenshot());
  await view.testId('lx-picker-cancel').click();
  await expect(view.testId('lx-picker-cancel')).toHaveCount(0);
  await remind.fill('07:30');
  await expect(view.testId('remind-value')).toHaveText('Set for 07:30');
});

test('playground: a changed prop redraws the component and rewrites the code', { id: 'PLAYGROUND-001', start: { page: 'playground' }, timeout: 90_000 }, async (t) => {
  const view = t.app.view;
  const stage = view.testId('playground-stage');
  const code = view.testId('playground-code');
  await expect(stage.getByRole('button', { name: 'Save changes' })).toBeEnabled();
  await view.getByRole('textbox', { name: 'label' }).fill('Send');
  await expect(stage.getByRole('button', { name: 'Send' })).toBeAttached();
  await expect(code).toContainText('label="Send"');
  await view.getByRole('switch', { name: 'isDisabled' }).click();
  await expect(code).toContainText('isDisabled');
  await t.attach('playground-button.png', await view.screenshot());
  await view.testId('playground-reset').click();
  await expect(stage.getByRole('button', { name: 'Save changes' })).toBeEnabled();
  await view.getByRole('combobox', { name: 'Component' }).selectOption('Badge');
  await expect(code).toContainText('<Badge');
  await expect(stage).toContainText('In review');
  await t.attach('playground-badge.png', await view.screenshot());
});

test('dropdown menu: on a narrow screen Escape leaves a submenu before it closes the menu', { id: 'SUBMENU-ESCAPE-001', start: { page: 'dropdown-menu' }, timeout: 90_000 }, async (t) => {
  const view = t.app.view;
  // Beside its item at regular width, where there is no level to go back to.
  if ((await view.viewport()).width >= 600) return;
  await view.testId('note-menu').click();
  await view.testId('move').click();
  const back = view.testId('move-back');
  await expect(back).toBeVisible();
  await back.press('Escape');
  await expect(back).toHaveCount(0);
  await expect(view.testId('move')).toBeVisible();
  await view.testId('move').press('Escape');
  await expect(view.testId('move')).toHaveCount(0);
});

test('tree list: Right opens a closed branch and Left closes it', { id: 'TREE-KEYS-001', start: { page: 'tree-list' }, timeout: 60_000 }, async (t) => {
  const shared = t.app.view.testId('folders-shared');
  await expect(shared).not.toBeExpanded();
  await shared.press('ArrowRight');
  await expect(shared).toBeExpanded();
  await shared.press('ArrowLeft');
  await expect(shared).not.toBeExpanded();
});

test('dashboard: every period draws its charts', { id: 'DASHBOARD-PERIODS-001', start: { page: 'dashboard' }, timeout: 90_000 }, async (t) => {
  const view = t.app.view;
  // A page that fails to render answers every locator with the failure, so each check below also says the page is alive.
  for (const period of ['month', 'week', 'year']) {
    await view.testId(`period-${period}`).click();
    await expect(view.testId(`period-${period}`)).toBeChecked();
    await expect(view.testId('revenue-chart')).toBeAttached();
    await expect(view.testId('products')).toBeAttached();
  }
  await t.attach('dashboard-year.png', await view.screenshot());
});

test('context menu (touch): a long press docks the menu level between the edges, and an item runs its action', { id: 'CONTEXT-MENU-TOUCH-001', start: { page: 'context-menu' }, timeout: 60_000 }, async (t) => {
  // A desktop asks with a secondary click, which no spec can send; there a held touch is a press.
  await touchMinimum(t);
  const view = t.app.view;
  const centre = (box: Box): [number, number] => [box.x + box.width / 2, box.y + box.height / 2];
  const status = view.getByRole('status').first();
  // A plain tap opens the note, never the menu.
  await tap(view, centre(await view.testId('note-coast').boundingBox()));
  await expect(status).toHaveText('Opened Trip to the coast');
  await expect(view.testId('note-coast-menu')).toHaveCount(0);
  const finger = await view.touch(centre(await view.testId('note-reading').boundingBox()));
  await pause(900);
  await finger.up();
  await pause(600);
  const sheet = await view.testId('note-reading-menu').boundingBox();
  const page = await view.testId('note-reading-menu-backdrop').boundingBox();
  const [before, after] = [sheet.x - page.x, page.x + page.width - (sheet.x + sheet.width)];
  if (Math.abs(before - after) > 1) throw Error(`the docked menu is ${before} from one edge and ${after} from the other`);
  await t.attach('context-menu-docked.png', await view.screenshot());
  // A real tap on an item: it must reach the item, not the layer under the menu.
  await tap(view, centre(await view.testId('pin-reading').boundingBox()));
  await expect(status).toHaveText('Pinned Reading list');
  await expect(view.testId('note-reading-menu')).toHaveCount(0);
});

test('conversation: a message is sent, a failed one says so and is sent again', { id: 'CONVERSATION-001', start: { page: 'conversation' }, timeout: 60_000 }, async (t) => {
  const view = t.app.view;
  const field = view.testId('composer');
  const send = view.testId('composer-send');
  await expect(send).toBeDisabled();
  await field.fill('I can see it from here.');
  await expect(send).toBeEnabled();
  await send.click();
  // Ten entries are seeded, so the new message is the eleventh.
  await expect(view.testId('message-m11')).toBeAttached();
  await expect(field).toHaveValue('');
  // The message that failed says so, and its own button sends it again.
  await expect(view.testId('retry-m8')).toBeAttached();
  await view.testId('retry-m8').click();
  await expect(view.testId('retry-m8')).toHaveCount(0);
  await t.attach('conversation-sent.png', await view.screenshot());
});

test('composer: past one line the text takes the full width and the buttons go under it; five lines at most, whole', { id: 'COMPOSER-STACK-001', start: { page: 'composer' }, timeout: 60_000 }, async (t) => {
  const view = t.app.view;
  const field = view.testId('reply');
  const send = view.testId('reply-send');
  const boxes = async () => ({ field: await field.boundingBox(), send: await send.boundingBox() });
  await field.fill('Leaving at eight');
  await pause(400);
  const row = await boxes();
  const line = row.field.height;
  if (row.send.y >= row.field.y + line) throw Error('a short draft is one row: the send button sits beside the text');
  await field.fill('Leaving at eight from the east gate. Bring the tent, the stove, the maps and water for two days, and call if the road is closed.');
  await pause(500);
  const stacked = await boxes();
  if (stacked.send.y < stacked.field.y + stacked.field.height - 1) throw Error('a long draft is stacked: the send button sits under the text');
  if (stacked.field.width <= row.field.width) throw Error('stacked, the text is wider than beside the button');
  await field.fill(Array.from({ length: 9 }, (_, index) => `Line ${index + 1}`).join('\n'));
  await pause(500);
  const full = await boxes();
  const lines = full.field.height / line;
  if (Math.abs(lines - 5) > 0.05) throw Error(`nine lines show ${lines} lines' height; five whole lines are the most`);
  await t.attach('composer-five-lines.png', await view.screenshot());
  await field.fill('Short again');
  await pause(500);
  const back = await boxes();
  if (back.send.y >= back.field.y + line) throw Error('a short draft is one row again');
});
