import { expect, spec } from '@lingxia/test';
import { openDemo } from './open-demo';
import { apart, fillOf } from './pixels';

// GPU-only fixtures read through role and accessible name, the way a screen reader does.
spec('the catalogue answers locators by role and name', { start: openDemo('names') }, async (t) => {
  const view = t.app.view;
  await expect(view.getByRole('heading', { name: 'Monthly bills' })).toBeVisible();
  await expect(view.testId('lab-a11y-list').getByRole('listitem')).toHaveCount(3);
  const status = view.getByRole('status');
  await expect(status).toHaveText('Not saved yet');
  await view.getByRole('button', { name: 'Save' }).click();
  await expect(status).toHaveText('Saved 1 times');
});

spec('composites take one Tab stop, arrows move between items and tabs activate on arrival', {
  start: openDemo('groups'),
}, async (t) => {
  const view = t.app.view;
  await expect(view.getByRole('tablist', { name: 'Range' })).toHaveCount(1);
  const week = view.getByRole('tab', { name: 'Week' });
  await week.press('ArrowRight');
  await expect(view.getByRole('tab', { name: 'Month' })).toBeFocused();
  await expect(view.testId('lab-range-panel')).toHaveText('Showing the month');
  await view.getByRole('tab', { name: 'Month' }).press('ArrowRight');
  await expect(view.testId('lab-range-panel')).toHaveText('Showing the day');
  await view.testId('lab-tool-bold').press('ArrowRight');
  await expect(view.testId('lab-tool-copy')).toBeFocused();
  await view.testId('lab-tree-src').press('ArrowRight');
  await expect(view.testId('lab-tree-main')).toBeFocused();
  await view.testId('lab-tree-main').press('ArrowLeft');
  await expect(view.testId('lab-tree-src')).toBeFocused();
  await expect(view.getByRole('treeitem', { name: 'main.ts' })).toHaveCount(1);
});

spec('focus-visible paints after a key moves focus and clears after a finger tap', { start: openDemo('focus') }, async (t) => {
  const view = t.app.view;
  const ring = view.testId('lab-focus-visible');
  const plain = await fillOf(t, ring);
  await view.testId('lab-focus-open').press('Tab');
  await expect(ring).toBeFocused();
  const keyed = await fillOf(t, ring);
  expect(apart(keyed, plain) ? 'painted' : `plain ${JSON.stringify({ keyed, plain })}`).toBe('painted');
  // A finger, not click(): a click activates without touching keyboard focus, so the ring would stay.
  await (await view.touch(ring)).up();
  await expect.poll(async () => apart(await fillOf(t, ring), plain), { timeout: 3_000 }).toBe(false);
});

spec('a hint shows after a key rests focus on its trigger and Escape hides it', { start: openDemo('focus') }, async (t) => {
  const view = t.app.view;
  const hint = view.testId('lab-interest-hint');
  await expect(hint).toHaveCount(0);
  await view.testId('lab-focus-visible').press('Tab');
  const trigger = view.testId('lab-interest');
  await expect(trigger).toBeFocused();
  await expect(hint).toBeVisible();
  await expect(trigger).toHaveAttribute('aria-describedby', 'lab-interest-hint');
  await trigger.press('Escape');
  await expect(hint).toHaveCount(0);
  await expect(trigger).toBeFocused();
});

spec('controls report checked, selected, pressed and expanded as a screen reader announces them', { covers: ['view:switch', 'view:checkbox', 'view:radio', 'view:a11y'], start: openDemo('nested') }, async (t) => {
  const view = t.app.view;
  const wifi = view.testId('lab-nested-switch');
  await expect(wifi).toBeChecked();
  await wifi.click();
  await expect(wifi).not.toBeChecked();
  await t.app.nav.to(openDemo('controls'));
  await expect(view.testId('lab-controls-size-m')).toBeChecked();
  await view.testId('lab-controls-size-l').click();
  await expect(view.testId('lab-controls-size-l')).toBeChecked();
  await expect(view.testId('lab-controls-size-m')).not.toBeChecked();
  await t.app.nav.to(openDemo('groups'));
  await expect(view.getByRole('tab', { name: 'Week' })).toBeSelected();
  await view.getByRole('tab', { name: 'Day' }).click();
  await expect(view.getByRole('tab', { name: 'Week' })).not.toBeSelected();
  const bold = view.testId('lab-tool-bold');
  await expect(bold).not.toBePressed();
  await bold.click();
  await expect(bold).toBePressed();
  const src = view.testId('lab-tree-src');
  await expect(src).toBeExpanded();
  await src.click();
  await expect(src).not.toBeExpanded();
  // A plain button has none of these states: asserting one is a wrong locator, not a pass.
  const refused = await expect(view.testId('lab-tool-copy')).not.toBeChecked({ timeout: 300 }).then(() => '', (error: Error) => error.message);
  expect(refused).toContain('checked state');

  await t.app.nav.to(openDemo('section'));
  const summary = view.testId('lab-disclosure-licenses-summary');
  await expect(summary).not.toBeExpanded();
  await summary.click();
  await expect(summary).toBeExpanded();
});
