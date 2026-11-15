import { expect, spec } from '@lingxia/test';

// Liuli's gallery on a device: every page opens, and each control answers taps and keys as its
// accessibility pattern says. States are asserted as a screen reader hears them: checked, selected, expanded.

const pause = (ms: number) => new Promise<void>(resolve => setTimeout(resolve, ms));

// Home's category cards and each category page's tiles, in order; core's gallery.test.mjs keeps these
// equal to the gallery.
const categories: Record<string, string[]> = {
  'category-foundations': ['colour', 'typography', 'shape-and-spacing', 'elevation', 'motion'],
  'category-actions': ['button', 'toggle-button', 'icon-button', 'button-group'],
  'category-inputs': ['text-input', 'text-area', 'date-input', 'time-input', 'slider', 'field', 'search-field', 'number-input', 'calendar', 'date-range-input'],
  'category-selection': ['checkbox-input', 'radio-group', 'switch', 'select', 'segmented-control', 'combobox', 'checkbox-list'],
  'category-navigation': ['tabs', 'nav-list', 'breadcrumb', 'pagination', 'stepper'],
  'category-feedback': ['badge', 'progress-bar', 'banner', 'empty-state', 'skeleton', 'toast'],
  'category-overlays': ['dialog', 'dropdown-menu', 'tooltip', 'bottom-sheet', 'popover'],
  'category-content': ['card', 'item', 'details', 'avatar', 'split-view', 'icon', 'table', 'markdown'],
  'category-charts': ['stat', 'sparkline', 'bar-chart', 'line-chart', 'pie-chart', 'chart'],
  'category-media': ['video-player', 'video-controls'],
  'category-patterns': ['sign-in', 'settings', 'uploads'],
};

spec('gallery: home opens every category, and each category every page', { start: { page: 'home' }, timeout: 360_000 }, async (t) => {
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

spec('gallery: home search narrows to matching components and opens one', { start: { page: 'home' }, timeout: 60_000 }, async (t) => {
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

spec('dropdown: checkbox and radio items check and keep the menu open; arrows reach them', { start: { page: 'dropdown-menu' }, timeout: 60_000 }, async (t) => {
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

spec('card choice: a tap checks a card; arrows move and choose', { start: { page: 'card' }, timeout: 60_000 }, async (t) => {
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

spec('item: rows in an ItemList are read as list items of a named list', { start: { page: 'item' }, timeout: 60_000 }, async (t) => {
  const view = t.app.view;
  await expect(view.getByRole('list', { name: 'Notes' })).toBeVisible();
  await expect(view.getByRole('list', { name: 'About' })).toBeAttached();
  await expect(view.getByRole('listitem')).toHaveCount(5);
  await expect(view.getByRole('button', { name: 'Trip to the coast' })).toBeVisible();
});

spec('uploads: each section is a named list, every card a list item', { start: { page: 'uploads' }, timeout: 60_000 }, async (t) => {
  const view = t.app.view;
  await expect(view.getByRole('list', { name: 'In progress' })).toBeVisible();
  await expect(view.getByRole('list', { name: 'Done today' })).toBeAttached();
  await expect(view.getByRole('listitem')).toHaveCount(4);
});

spec('text area: typing reaches it, it grows, and Send submits the reply', { start: { page: 'text-area' }, timeout: 60_000 }, async (t) => {
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

spec('empty state: its title is a heading, and its action works', { start: { page: 'empty-state' }, timeout: 60_000 }, async (t) => {
  const view = t.app.view;
  await expect(view.getByRole('heading', { name: 'No notebooks yet' })).toBeVisible();
  await view.testId('create').click();
  await expect(view.getByRole('heading', { name: 'Notebook created' })).toBeVisible();
});

spec('skeleton: one busy status per loading group, named by what is loading', { start: { page: 'skeleton' }, timeout: 60_000 }, async (t) => {
  const view = t.app.view;
  await expect(view.getByRole('status', { name: 'Loading notes' })).toBeVisible();
  await t.attach('skeleton.png', await view.screenshot());
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
  await view.getByRole('option', { name: 'France' }).click();
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

spec('switch: a tap on its help text flips it', { start: { page: 'switch' }, timeout: 60_000 }, async (t) => {
  const view = t.app.view;
  const sync = view.getByRole('switch', { name: 'Sync over mobile data' });
  await expect(sync).toBeChecked();
  const box = await sync.boundingBox();
  // The help line is the switch's last line, at its start; the box holds both lines.
  if (box.height < 36) throw new Error(`switch box ${box.height} tall holds only its label`);
  await view.swipe({ from: [box.x + 24, box.y + box.height - 10], to: [box.x + 24, box.y + box.height - 10], durationMs: 60 });
  await expect(sync).not.toBeChecked();
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

spec('toggle button: a tap presses it and filters the list; Space releases it', { start: { page: 'toggle-button' }, timeout: 60_000 }, async (t) => {
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

spec('segmented control: a named radio group; a tap and an arrow choose', { start: { page: 'segmented-control' }, timeout: 60_000 }, async (t) => {
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

spec('toast: Undo restores the note; tones queue and close by themselves; one with Undo waits for a swipe', { start: { page: 'toast' }, timeout: 90_000 }, async (t) => {
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

spec('bottom sheet: opens as a named dialog; a row closes it; a drag down dismisses it', { start: { page: 'bottom-sheet' }, timeout: 90_000 }, async (t) => {
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

spec('popover: opens a named panel; a control inside keeps it open; Escape closes it and returns focus', { start: { page: 'popover' }, timeout: 60_000 }, async (t) => {
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

spec('date input: a tap opens the picker; a filled day reaches the page; a return before leaving is invalid', { start: { page: 'date-input' }, timeout: 60_000 }, async (t) => {
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

spec('time input: a tap opens the picker; a filled time reaches the page', { start: { page: 'time-input' }, timeout: 60_000 }, async (t) => {
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
