import { expect, test } from '@lingxia/test';
import { openDemo } from './open-demo';

// An element asks for its own menu; the page opens it.
test('a row opens its menu when asked, not on a tap, and a choice closes it', { covers: ['view:context-menu'], start: openDemo('menu') }, async (t) => {
  const view = t.app.view;
  const row = view.testId('lab-menu-row');
  const state = view.testId('lab-menu-state');
  await expect(state).toHaveAttribute('data-state', 'open=false asked=none last=none');
  // As the Menu key would ask: from the keyboard, at the row.
  await row.performAction('contextmenu');
  await expect(state).toHaveAttribute('data-state', 'open=true asked=keyboard last=none');
  await expect(view.testId('lab-menu-list')).toBeVisible();
  await view.testId('lab-menu-rename').click();
  await expect(state).toHaveAttribute('data-state', 'open=false asked=keyboard last=rename');
});

// One Tab stop; arrows by day and by week; PageDown turns the month.
test('arrows move through the days by row and column and PageDown turns the month', { covers: ['view:grid'], start: openDemo('days') }, async (t) => {
  const view = t.app.view;
  const state = view.testId('lab-days-state');
  const day = (date: number) => view.testId(`lab-day-${date}`);
  await expect(state).toHaveAttribute('data-state', 'month=0 day=14');
  await day(14).press('ArrowRight');
  await expect(day(15)).toBeFocused();
  await day(15).press('ArrowDown');
  await expect(day(22)).toBeFocused();
  await day(22).press('ArrowUp');
  await expect(day(15)).toBeFocused();
  // The first week starts with days of the month before: Home lands on the first of this one.
  await day(3).press('Home');
  await expect(day(1)).toBeFocused();
  // Up from the first week has nowhere to go.
  await day(1).press('ArrowUp');
  await expect(day(1)).toBeFocused();
  await day(1).press('PageDown');
  await expect(state).toHaveAttribute('data-state', 'month=1 day=14');
  // The new month starts a weekday later, so the first of it moved one column over.
  await day(1).press('Home');
  await expect(day(1)).toBeFocused();
});
