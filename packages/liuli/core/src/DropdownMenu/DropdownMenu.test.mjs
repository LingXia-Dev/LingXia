import { test } from 'node:test';
import assert from 'node:assert/strict';
import { textOf } from '@lingxia/view/testing';
import { renderExample, renderPage } from '../testing/harness.mjs';

const menus = screen => screen.queryAll(node => node.props.role === 'menu');

test('Basic: the trigger opens a menu named by the label; an item runs and closes it', async () => {
  const screen = await renderExample('DropdownMenu/DropdownMenu', 'Basic');
  assert.equal(screen.getByTestId('sort').props.accessibilityLabel, 'Sort by');
  assert.equal(menus(screen).length, 0);
  screen.press(screen.getByTestId('sort'));
  assert.equal(menus(screen).length, 1);
  assert.equal(menus(screen)[0].props.accessibilityLabel, 'Sort by');
  const items = screen.queryAll(node => node.props.role === 'menuitem');
  assert.deepEqual(items.map(textOf), ['Name', 'Last updatedNewest first', 'Size']);
  screen.press(screen.getByTestId('sort-updated'));
  assert.equal(textOf(screen.getByTestId('order')), 'Last updated');
  assert.equal(menus(screen).length, 0);
});

test('More: an icon-only square trigger; a disabled item takes no press; the divider is hidden', async () => {
  const screen = await renderExample('DropdownMenu/DropdownMenu', 'More');
  const trigger = screen.getByTestId('more');
  assert.equal(trigger.props.accessibilityLabel, 'More actions');
  assert.equal(trigger.props.minHeight, undefined);
  assert.equal(trigger.props.width, 40);
  screen.press(trigger);
  const share = screen.getByText('Share');
  let item = share;
  while (item.tag !== 'button') item = item.parent;
  assert.equal(item.props.onClick, undefined);
  assert.ok(menus(screen)[0].children.some(node => node.props.accessibilityHidden === 1 && node.children.length === 0), 'divider');
  screen.press(screen.getByTestId('delete'));
  assert.equal(screen.getByText('Removed').tag, 'text');
});

test('InList: each row has its own menu, without caller ids', async () => {
  const screen = await renderExample('DropdownMenu/DropdownMenu', 'InList');
  screen.press(screen.getByTestId('menu-Archive'));
  assert.equal(menus(screen).length, 1);
  assert.equal(menus(screen)[0].props.accessibilityLabel, 'Archive actions');
});

test('an item outside a DropdownMenu fails, naming the fix', async () => {
  await assert.rejects(
    renderPage(`
      import { DropdownMenuItem } from '@liulidesign/core/DropdownMenu';
      export function render() { return <page><DropdownMenuItem label="Lost" onClick={() => {}} /></page>; }
    `),
    /DropdownMenuItem: place it inside a DropdownMenu/,
  );
});
