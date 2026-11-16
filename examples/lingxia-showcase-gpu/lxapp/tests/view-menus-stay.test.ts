import { expect, spec } from '@lingxia/test';
import { openDemo } from './open-demo';

// A menu item may act and leave its menu open; the others close it as before.
spec('a keep-open item acts twice with its menu still open, and a plain item closes it', { covers: ['view:menu-keep-open'], start: openDemo('stay') }, async (t) => {
  const view = t.app.view;
  const state = view.testId('lab-stay-state');
  const menu = view.testId('lab-stay-menu');
  await view.testId('lab-stay-open').click();
  await expect(menu).toBeVisible();
  await view.testId('lab-stay-in').click();
  await view.testId('lab-stay-in').click();
  await expect(state).toHaveAttribute('data-state', 'open=true zoom=120');
  await expect(menu).toBeVisible();
  // From the keyboard the item keeps focus as well as its menu.
  await view.testId('lab-stay-in').press('ArrowDown');
  await expect(view.testId('lab-stay-out')).toBeFocused();
  await view.testId('lab-stay-out').press('Enter');
  await expect(state).toHaveAttribute('data-state', 'open=true zoom=110');
  await expect(view.testId('lab-stay-out')).toBeFocused();
  await view.testId('lab-stay-reset').click();
  await expect(state).toHaveAttribute('data-state', 'open=false zoom=100');
  await expect(menu).toBeHidden();
});

// Escape asks the popover first, each time; a press outside does not ask.
spec('a popover refuses Escape while its draft is kept, and closes once it is not', { covers: ['view:close-request'], start: openDemo('refuse') }, async (t) => {
  const view = t.app.view;
  const state = view.testId('lab-refuse-state');
  const note = view.testId('lab-refuse-note');
  const keep = view.testId('lab-refuse-keep');
  await view.testId('lab-refuse-open').click();
  await expect(note).toBeVisible();
  await keep.press('Escape');
  await expect(state).toHaveAttribute('data-state', 'open=true asked=1');
  // Assistive tech's dismiss asks the same question as Escape.
  await note.performAction('dismiss');
  await expect(state).toHaveAttribute('data-state', 'open=true asked=2');
  await expect(note).toBeVisible();
  await keep.click();
  await keep.press('Escape');
  await expect(state).toHaveAttribute('data-state', 'open=false asked=3');
  await expect(note).toBeHidden();
});
