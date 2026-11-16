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
  // Beside the first menu; in a window with room on neither side, under its item.
  const [first, second, opener] = [await menu.boundingBox(), await folders.boundingBox(), await move.boundingBox()];
  expect(second.x >= first.x + first.width - 1 || second.y >= opener.y + opener.height).toBe(true);
  await view.testId('lab-submenu-inbox').press('ArrowLeft');
  await expect(folders).toBeHidden();
  await expect(menu).toBeVisible();
  await move.press('ArrowRight');
  await expect(folders).toBeVisible();
  await expect(view.testId('lab-submenu-inbox')).toBeFocused();
  // Opened from the keyboard, it hands focus back to the item that opened it.
  await view.testId('lab-submenu-inbox').press('ArrowLeft');
  await expect(folders).toBeHidden();
  await expect(move).toBeFocused();
  await move.press('ArrowRight');
  await expect(view.testId('lab-submenu-inbox')).toBeFocused();
  await view.testId('lab-submenu-inbox').press('ArrowDown');
  await expect(view.testId('lab-submenu-archive')).toBeFocused();
  await view.testId('lab-submenu-archive').press('Enter');
  await expect(state).toHaveAttribute('data-state', 'last=archive');
  await expect(folders).toBeHidden();
  await expect(menu).toBeHidden();
});

// Opened by a press on its item, the second menu hands focus back to that item too.
spec('a second menu a press opened returns focus to its item when the back arrow closes it', { covers: ['view:submenu'], start: openDemo('submenu') }, async (t) => {
  const view = t.app.view;
  const move = view.testId('lab-submenu-move');
  await view.testId('lab-submenu-open').click();
  await move.click();
  await expect(view.testId('lab-submenu-folders')).toBeVisible();
  await view.testId('lab-submenu-inbox').press('ArrowLeft');
  await expect(view.testId('lab-submenu-folders')).toBeHidden();
  await expect(view.testId('lab-submenu-menu')).toBeVisible();
  await expect(move).toBeFocused();
});
