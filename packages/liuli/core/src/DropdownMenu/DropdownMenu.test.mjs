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
  assert.ok(menus(screen)[0].children[0].children.some(node => node.props.accessibilityHidden === 1 && node.children.length === 0), 'divider');
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

test('Value: the trigger is the author\'s Button showing the choice; the menu keeps the setting\'s name', async () => {
  const screen = await renderExample('DropdownMenu/DropdownMenu', 'Value');
  const trigger = screen.getByTestId('appearance');
  assert.equal(trigger.props.accessibilityLabel, 'System');
  screen.press(trigger);
  assert.equal(screen.queryAll(node => node.props.role === 'menu' && node.props.accessibilityLabel === 'Appearance').length, 1);
  screen.press(screen.getByTestId('appearance-Dark'));
  assert.equal(screen.getByTestId('appearance').props.accessibilityLabel, 'Dark');
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

test('StateMixed: a set partly on reads as mixed with a dash; a press turns all on, then all off', async () => {
  const screen = await renderExample('DropdownMenu/DropdownMenu', 'StateMixed');
  screen.press(screen.getByTestId('labels-menu'));
  const all = () => screen.getByTestId('labels-all');
  assert.deepEqual([all().props.stateChecked, all().props.mixed], [0, 1]);
  assert.ok(all().children.some(node => node.tag !== 'vector' && node.props.accessibilityHidden && !node.children.length), 'a hidden dash');
  assert.ok(!all().children.some(node => node.tag === 'vector'), 'no check while mixed');
  screen.press(all());
  assert.deepEqual([all().props.stateChecked, all().props.mixed], [1, undefined]);
  assert.equal(screen.getByTestId('label-Travel').props.stateChecked, 1);
  screen.press(all());
  assert.deepEqual([all().props.stateChecked, all().props.mixed], [0, undefined]);
  assert.equal(screen.getByTestId('label-Work').props.stateChecked, 0);
});

test('a radio item outside a DropdownMenuRadioGroup fails, naming the fix', async () => {
  await assert.rejects(async () => {
    const screen = await renderPage(`
      import { Button } from '@liulidesign/core/Button';
      import { DropdownMenu, DropdownMenuRadioItem } from '@liulidesign/core/DropdownMenu';
      export function render() {
        return <page><DropdownMenu label="Menu" menu={<DropdownMenuRadioItem value="a" label="A" />}><Button label="Menu" data-testid="menu" /></DropdownMenu></page>;
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

test('Submenu, wide: an item opens a second menu named by it, anchored at its side; choosing there runs the action', async () => {
  const screen = await renderExample('DropdownMenu/DropdownMenu', 'Submenu', { env: { viewport: { width: 1024, height: 768 } } });
  screen.press(screen.getByTestId('note-menu'));
  screen.press(screen.getByTestId('move'));
  const sub = screen.getByTestId('move-menu');
  assert.deepEqual([sub.props.role, sub.props.accessibilityLabel], ['menu', 'Move to']);
  assert.ok(JSON.stringify(sub.props).includes('inline-end span-block-end'));
  screen.press(screen.getByTestId('move-travel'));
  screen.getByText('In Travel');
});

test('Submenu, narrow: the submenu takes the panel under a row that goes back; choosing there closes the menu', async () => {
  const screen = await renderExample('DropdownMenu/DropdownMenu', 'Submenu');
  screen.press(screen.getByTestId('note-menu'));
  const labels = () => menus(screen)[0].children[0].children.filter(node => node.tag === 'button' || node.props.role === 'group').map(node => node.props.accessibilityLabel);
  assert.deepEqual(labels(), ['Rename', 'Move to', 'Delete']);
  screen.press(screen.getByTestId('move'));
  assert.equal(menus(screen).length, 1, 'the same panel');
  assert.deepEqual(labels(), ['Move to', 'Move to'], 'the back row, then the submenu as a group');
  assert.equal(screen.getByTestId('move-back').props.stateExpanded, 1);
  assert.equal(screen.getByTestId('move-back').props.role, 'menuitem', 'reached by the arrow keys like any row');
  screen.press(screen.getByTestId('move-back'));
  assert.deepEqual(labels(), ['Rename', 'Move to', 'Delete']);
  screen.press(screen.getByTestId('move'));
  screen.press(screen.getByTestId('move-recipes'));
  assert.equal(menus(screen).length, 0);
  screen.getByText('In Recipes');
});

test('a trigger that is not a Liuli button is wrapped in one plain button that opens the menu', async () => {
  const screen = await renderPage(`
    import { DropdownMenu, DropdownMenuItem } from '@liulidesign/core/DropdownMenu';
    export function render() {
      return <page><DropdownMenu label="Account" menu={<DropdownMenuItem label="Sign out" onClick={() => {}} />} data-testid="account"><text>Ana Lima</text></DropdownMenu></page>;
    }
  `);
  const trigger = screen.getByTestId('account');
  assert.equal(trigger.tag, 'button');
  assert.equal(screen.queryAll(node => node.tag === 'button').length, 1, 'one focus stop');
  screen.press(trigger);
  assert.equal(screen.getByTestId('account-menu').props.role, 'menu');
});

test('more than one element as the trigger fails, naming the fix', async () => {
  await assert.rejects(async () => {
    await renderPage(`
      import { DropdownMenu, DropdownMenuItem } from '@liulidesign/core/DropdownMenu';
      export function render() {
        return <page><DropdownMenu label="Account" menu={<DropdownMenuItem label="Sign out" onClick={() => {}} />}><text>Ana</text><text>Lima</text></DropdownMenu></page>;
      }
    `);
  }, /one element as its trigger/);
});

test('a Link mid-sentence is the trigger and opens the menu', async () => {
  const screen = await renderPage(`
    import { DropdownMenu } from '@liulidesign/core/DropdownMenu';
    import { DropdownMenuItem } from '@liulidesign/core/DropdownMenu';
    import { Link } from '@liulidesign/core/Link';
    export function render() { return <page><text>Due <DropdownMenu label="When" data-testid="menu" menu={<DropdownMenuItem label="Move to Monday" onClick={() => {}} />}><Link label="Sunday" data-testid="when" /></DropdownMenu>, at six.</text></page>; }
  `);
  const link = screen.getByTestId('when');
  assert.equal(link.tag, 'link');
  assert.equal(screen.queryAll(node => node.tag === 'text' && node.children?.includes(link)).length, 1, 'the link stays in its sentence');
  assert.equal(link.props.stateExpanded, 0);
  screen.press(link);
  assert.equal(screen.getByTestId('when').props.stateExpanded, 1);
  assert.equal(screen.queryAll(node => node.props.role === 'menu').length, 1);
});

test('InSentence: the link stays in its sentence and wires the overlay', async () => {
  const screen = await renderExample('DropdownMenu/DropdownMenu', 'InSentence');
  const link = screen.queryAll(node => node.tag === 'link')[0];
  assert.ok(link, 'a link');
  assert.equal(screen.queryAll(node => node.tag === 'text' && node.children?.includes(link)).length, 1, 'inside the sentence');
  assert.ok(link.props.interestFor || link.props.stateExpanded === 0, 'wired to its overlay');
});

test('Data: rows given as data draw as the composed ones: a submenu, a named group, dividers and a destructive action', async () => {
  const screen = await renderExample('DropdownMenu/DropdownMenu', 'Data');
  screen.press(screen.getByTestId('data'));
  const top = menus(screen)[0];
  assert.equal(top.props.accessibilityLabel, 'Note actions');
  const labels = screen.queryAll(node => node.props.role === 'menuitem').map(node => node.props.accessibilityLabel);
  assert.deepEqual(labels.slice(0, 6), ['Rename', 'Duplicate', 'Move to', 'Copy link', 'Invite someone', 'Delete']);
  const group = screen.queryAll(node => node.props.role === 'group' && node.props.accessibilityLabel === 'Share');
  assert.equal(group.length, 1, 'the group is named by its label');
  const invite = screen.queryAll(node => node.props.role === 'menuitem' && node.props.accessibilityLabel === 'Invite someone')[0];
  assert.equal(invite.props.onClick, undefined, 'a disabled row takes no press');
  screen.press(screen.getByTestId('data-delete'));
  assert.equal(menus(screen).length, 0, 'choosing closes the menu');
});

test('Data: a row with items opens them as its submenu, and choosing there runs the action', async () => {
  const screen = await renderExample('DropdownMenu/DropdownMenu', 'Data');
  screen.press(screen.getByTestId('data'));
  screen.press(screen.getByTestId('data-move'));
  assert.ok(screen.getByTestId('data-move-menu'));
  screen.press(screen.getByTestId('data-inbox'));
  assert.ok(screen.getByText('Moved to Inbox'));
});

test('items: rows key by their place, so two rows may share a label; two rows with one id are told about', async () => {
  const warned = [];
  const warn = console.warn;
  console.warn = message => warned.push(message);
  try {
    const screen = await renderPage(`
      import { DropdownMenu } from '@liulidesign/core/DropdownMenu';
      import { Button } from '@liulidesign/core/Button';
      export function render() {
        return <page><DropdownMenu label="Devices" items={[
          { type: 'group', label: 'iPad', items: [{ label: 'Remove', onClick: () => {} }] },
          { type: 'group', label: 'Phone', items: [{ label: 'Remove', onClick: () => {} }] },
          { label: 'Rename', id: 'same', onClick: () => {} },
          { label: 'Move', id: 'same', onClick: () => {} },
        ]}><Button label="Devices" data-testid="m" /></DropdownMenu></page>;
      }
    `);
    screen.press(screen.getByTestId('m'));
    const labels = screen.queryAll(node => node.props.role === 'menuitem').map(node => node.props.accessibilityLabel);
    assert.deepEqual(labels, ['Remove', 'Remove', 'Rename', 'Move']);
    assert.ok(warned.some(message => message.includes('two rows have the id "same"')), warned.join('\n'));
  } finally {
    console.warn = warn;
  }
});

test('Basic: the menu is as wide as its widest item, at least 192 pt and at most 90 % of the screen; an item\'s label counts toward that width', async () => {
  const screen = await renderExample('DropdownMenu/DropdownMenu', 'Basic');
  screen.press(screen.getByTestId('sort'));
  const [menu] = menus(screen);
  assert.deepEqual([menu.props.minWidth, menu.props.maxWidth, menu.props.width], [192, 351, undefined], 'at least 192 pt, at most 90 % of the 390 pt screen');
  const texts = screen.queryAll(node => node.tag === 'text' && ['Name', 'Size'].includes(node.text) && node.parent.parent.props.role === 'menuitem').map(label => label.parent);
  assert.deepEqual(texts.map(text => [text.props.flexBasis, text.props.minWidth]), [[undefined, 0], [undefined, 0]]);
});
