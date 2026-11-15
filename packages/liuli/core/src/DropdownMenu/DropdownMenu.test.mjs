import { test } from 'node:test';
import assert from 'node:assert/strict';
import { textOf } from '@lingxia/view/testing';
import { described, renderExample, renderPage } from '../testing/harness.mjs';

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

test('an item is named by its label alone; its second line is its description', async () => {
  const screen = await renderExample('DropdownMenu/DropdownMenu', 'Basic');
  screen.press(screen.getByTestId('sort'));
  const item = screen.getByTestId('sort-updated');
  assert.equal(item.props.accessibilityLabel, 'Last updated');
  assert.deepEqual(described(screen, item), ['Newest first']);
  assert.equal(screen.getByTestId('sort-name').props.ariaDescribedBy, undefined);
});

test('the menu panel is the raised surface with a hairline border and the raised shadow, so it separates from the page in dark themes', async () => {
  const screen = await renderExample('DropdownMenu/DropdownMenu', 'Basic');
  screen.press(screen.getByTestId('sort'));
  const menu = menus(screen)[0];
  assert.equal(menu.props.background, 'surface-overlay');
  assert.equal(menu.props.borderColor, 'text/12');
  assert.equal(menu.props.borderTopWidth, 1);
  assert.equal(JSON.parse(menu.props.boxShadow).length, 2, 'contact and ambient layers');
});

test('Value: the trigger shows the choice, is named by its label and described by the choice', async () => {
  const screen = await renderExample('DropdownMenu/DropdownMenu', 'Value');
  const trigger = screen.getByTestId('appearance');
  assert.equal(trigger.props.accessibilityLabel, 'Appearance');
  assert.deepEqual(described(screen, trigger), ['System']);
  screen.press(trigger);
  screen.press(screen.getByTestId('appearance-Dark'));
  assert.deepEqual(described(screen, screen.getByTestId('appearance')), ['Dark']);
});

test('Value: the chosen option is a checked radio item, and choosing another moves the check', async () => {
  const screen = await renderExample('DropdownMenu/DropdownMenu', 'Value');
  screen.press(screen.getByTestId('appearance'));
  const option = name => screen.getByTestId(`appearance-${name}`);
  assert.equal(option('System').props.role, 'menuitemradio');
  assert.equal(option('System').props.stateChecked, 1);
  assert.equal(option('Dark').props.stateChecked, 0);
  screen.press(option('Dark'));
  assert.equal(option('Dark').props.stateChecked, 1);
  assert.equal(option('System').props.stateChecked, 0);
});

test('Checkable: a checkbox item flips; a radio item checks one; the check is drawn only', async () => {
  const screen = await renderExample('DropdownMenu/DropdownMenu', 'Checkable');
  screen.press(screen.getByTestId('view-menu'));
  const completed = screen.getByTestId('show-completed');
  assert.equal(completed.props.role, 'menuitemcheckbox');
  assert.equal(completed.props.accessibilityLabel, 'Show completed');
  assert.equal(completed.props.stateChecked, 1);
  assert.ok(completed.children.some(node => node.tag === 'vector' && node.props.accessibilityHidden), 'a hidden check');
  assert.ok(!screen.getByTestId('show-archived').children.some(node => node.tag === 'vector'), 'no check while off');
  screen.press(completed);
  assert.equal(screen.getByTestId('show-completed').props.stateChecked, 0);
  screen.press(screen.getByTestId('group-Week'));
  assert.equal(screen.getByTestId('group-Week').props.stateChecked, 1);
  assert.equal(screen.getByTestId('group-Day').props.stateChecked, 0);
});

test('a radio item outside a DropdownMenuRadioGroup fails, naming the fix', async () => {
  await assert.rejects(async () => {
    const screen = await renderPage(`
      import { DropdownMenu, DropdownMenuRadioItem } from '@liulidesign/core/DropdownMenu';
      export function render() {
        return <page><DropdownMenu label="Menu" data-testid="menu"><DropdownMenuRadioItem value="a" label="A" /></DropdownMenu></page>;
      }
    `);
    screen.press(screen.getByTestId('menu'));
  }, /DropdownMenuRadioGroup/);
});

test('Grouped: each group is named by its heading, which is not read twice', async () => {
  const screen = await renderExample('DropdownMenu/DropdownMenu', 'Grouped');
  screen.press(screen.getByTestId('options'));
  const groups = screen.queryAll(node => node.props.role === 'group' && node.children[0]?.tag === 'text');
  assert.deepEqual(groups.map(group => [group.props.accessibilityLabel, group.children[0].props.accessibilityHidden]), [['Sort by', 1], ['Show', 1]]);
  assert.equal(screen.queryAll(node => node.props.role === 'menuitemradio').length, 3);
});
