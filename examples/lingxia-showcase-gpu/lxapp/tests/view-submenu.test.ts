import { expect, spec } from '@lingxia/test';
import { openDemo } from './open-demo';

// A menu item that opens a menu keeps its own menu open; a choice in the second closes both.
spec('a second menu opens beside the first, arrows open and close it, and a choice closes both', { covers: ['view:submenu'], start: openDemo('submenu') }, async (t) => {
  const view = t.app.view;
  const state = view.testId('lab-submenu-state');
  const menu = view.testId('lab-submenu-menu');
  const folders = view.testId('lab-submenu-folders');
  const move = view.testId('lab-submenu-move');
  await view.testId('lab-submenu-open').click();
  await expect(menu).toBeVisible();
  await move.click();
  await expect(folders).toBeVisible();
  await expect(menu).toBeVisible();
  // Beside the first menu, not over it.
  expect((await folders.boundingBox()).x).toBeGreaterThanOrEqual((await menu.boundingBox()).x + (await menu.boundingBox()).width - 1);
  await view.testId('lab-submenu-inbox').press('ArrowLeft');
  await expect(folders).toBeHidden();
  await expect(menu).toBeVisible();
  await expect(move).toBeFocused();
  await move.press('ArrowRight');
  await expect(folders).toBeVisible();
  await expect(view.testId('lab-submenu-inbox')).toBeFocused();
  await view.testId('lab-submenu-inbox').press('ArrowDown');
  await expect(view.testId('lab-submenu-archive')).toBeFocused();
  await view.testId('lab-submenu-archive').press('Enter');
  await expect(state).toHaveAttribute('data-state', 'last=archive');
  await expect(folders).toBeHidden();
  await expect(menu).toBeHidden();
});
