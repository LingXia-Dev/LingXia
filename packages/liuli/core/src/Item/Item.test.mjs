import { test } from 'node:test';
import assert from 'node:assert/strict';
import { textOf } from '@lingxia/view/testing';
import { described, renderExample, renderPage } from '../testing/harness.mjs';

test('Navigation: each row is one button named by its title, described by its second line, and runs once', async () => {
  const screen = await renderExample('Item/Item', 'Navigation');
  const row = screen.getByTestId('note-n2');
  assert.equal(row.tag, 'button');
  assert.equal(row.props.accessibilityLabel, 'Trip to the coast');
  assert.deepEqual(described(screen, row), ['Edited yesterday']);
  assert.ok(row.children.some(node => node.tag === 'vector' && node.props.accessibilityHidden), 'a hidden chevron');
  assert.equal(row.props.minHeight, 48);
});

test('ItemList: a named list whose rows sit in listitems, plain or pressable', async () => {
  for (const [example, testId, rows] of [['Navigation', 'note-n2', 3], ['Values', 'version', 2]]) {
    const screen = await renderExample('Item/Item', example);
    const [list] = screen.queryAll(node => node.props.role === 'list');
    assert.ok(list.props.accessibilityLabel, `${example}: the list is named`);
    const items = screen.queryAll(node => node.props.role === 'listitem');
    assert.equal(items.length, rows);
    assert.ok(items.some(item => item.children.includes(screen.getByTestId(testId))), `${example}: the row is a listitem's child`);
  }
});

test('an Item outside an ItemList is not wrapped', async () => {
  const screen = await renderExample('Item/Item', 'WithControl');
  assert.equal(screen.queryAll(node => node.props.role === 'listitem').length, 0);
});

test('a pressable row runs its onClick once per press', async () => {
  const screen = await renderPage(`
    import { Item } from '@liulidesign/core/Item';
    let presses = 0;
    export function render() {
      return <page>
        <Item label="Wi-Fi" onClick={() => { presses++; }} data-testid="row" />
        <text data-testid="presses">{String(presses)}</text>
      </page>;
    }
  `);
  screen.press(screen.getByTestId('row'));
  screen.rerender({});
  assert.equal(textOf(screen.getByTestId('presses')), '1');
});

test('Values: a plain row is not a button, and its end content stays readable', async () => {
  const screen = await renderExample('Item/Item', 'Values');
  const row = screen.getByTestId('version');
  assert.notEqual(row.tag, 'button');
  assert.equal(row.props.accessibilityLabel, undefined);
  const end = row.children.at(-1);
  assert.equal(end.props.accessibilityHidden, undefined);
  assert.equal(textOf(end), '2.4.1');
});

test('WithControl: the control at the end is its own button', async () => {
  const screen = await renderExample('Item/Item', 'WithControl');
  const action = screen.getByTestId('account-action');
  assert.equal(action.props.accessibilityLabel, 'Sign out');
  screen.press(action);
  assert.equal(screen.getByTestId('account-action').props.accessibilityLabel, 'Sign in');
  assert.ok(textOf(screen.getByTestId('account')).includes('Signed out'));
});

test('Disabled: takes no press and is described by the reason instead of its second line', async () => {
  const screen = await renderExample('Item/Item', 'Disabled');
  const row = screen.getByTestId('export');
  assert.equal(row.props.onClick, undefined);
  assert.deepEqual(described(screen, row), ['Available once a change is saved']);
  assert.equal(screen.queryAll(node => textOf(node) === 'Download every change as a file').length, 0);
});

test('Destructive: the danger tone colours the label only', async () => {
  const screen = await renderExample('Item/Item', 'Destructive');
  const row = screen.getByTestId('delete-row');
  assert.equal(row.props.accessibilityLabel, 'Delete account');
  assert.equal(screen.getByText('Delete account').props.color, 'danger');
  assert.equal(screen.getByText('Removes your notes from every device').props.color, 'text-muted');
});

test('Selection: the chosen row reads as selected, the others as not, and a press moves the choice', async () => {
  const screen = await renderExample('Item/Item', 'Selection');
  const selected = () => ['inbox', 'drafts', 'sent'].map(id => screen.getByTestId(`mailbox-${id}`).props.stateSelected);
  assert.deepEqual(selected(), [1, 0, 0]);
  screen.press(screen.getByTestId('mailbox-sent'));
  screen.rerender({});
  assert.deepEqual(selected(), [0, 0, 1]);
});

test('a row without isSelected has no selected state', async () => {
  const screen = await renderExample('Item/Item', 'Navigation');
  assert.equal(screen.getByTestId('note-n1').props.stateSelected, undefined);
});

test('Swipe: a row with actions slides over them; a reader is offered them by name, and running one closes the row first', async () => {
  const screen = await renderExample('Item/Item', 'Swipe');
  const slide = () => screen.getByTestId('mail-m2-swipe');
  assert.deepEqual(JSON.parse(slide().props.drag), { axis: 'x', points: [0, -144], index: 0 }, 'closed, or open by two actions of 72 px');
  assert.equal(slide().props.accessibilityLabel, 'Ben Okafor');
  assert.equal(slide().props.background, 'surface', 'the row covers its actions while it rests');
  const ids = String(slide().props.ariaActions).split(' ');
  assert.deepEqual(ids.map(id => textOf(screen.queryAll(node => node.props.elementId === id)[0])), ['Flag', 'Delete'], 'offered by their labels');
  const row = screen.getByTestId('mail-m2');
  assert.equal(row.tag, 'button');
  assert.equal(row.parent, slide(), 'the pressable row moves inside the slide');
  const remove = screen.getByTestId('mail-m2-delete');
  assert.deepEqual([remove.props.width, remove.props.background, remove.props.color], [72, 'danger', 'on-danger']);
  assert.equal(remove.children.filter(node => node.tag === 'vector').length, 1, 'an icon over the label');
  screen.press(screen.getByTestId('mail-m2-flag'));
  assert.equal(textOf(screen.getByTestId('mail-m2')).includes('Flagged'), true);
  screen.press(screen.getByTestId('mail-m2-delete'));
  assert.equal(screen.queryAll(node => node.testId === 'mail-m2').length, 0);
  assert.equal(screen.queryAll(node => node.props.role === 'listitem').length, 2);
});

test('ItemList: a title names the list and is drawn once; the line under it follows the rows', async () => {
  const screen = await renderExample('Item/Item', 'Swipe');
  const list = screen.getByTestId('inbox');
  assert.equal(list.props.role, 'list');
  assert.equal(list.props.accessibilityLabel, 'Inbox');
  const [title, , note] = list.parent.children;
  assert.deepEqual([title.text, title.props.accessibilityHidden, title.props.color, title.props.fontSize], ['Inbox', 1, 'text-muted', 13]);
  assert.equal(note.text, 'Drag a message to the side to flag or delete it.');
  const plain = await renderExample('Item/Item', 'Selection');
  assert.equal(plain.getByTestId('mailboxes').parent.children.length, 1, 'without a title or a description the list stands alone');
});

test('density: a compact list makes its rows 40 px; a row\'s own density wins', async () => {
  const screen = await renderExample('Item/Item', 'Compact');
  assert.equal(screen.getByTestId('file-Notes.md').props.minHeight, 40);
  const mixed = await renderPage(`
    import { Item, ItemList } from '@liulidesign/core/Item';
    export function render() {
      return <page><ItemList label="L" density="compact"><Item label="A" data-testid="a" /><Item label="B" density="comfortable" data-testid="b" /></ItemList><Item label="C" data-testid="c" /></page>;
    }
  `);
  assert.deepEqual(['a', 'b', 'c'].map(id => mixed.getByTestId(id).props.minHeight), [40, 48, 48]);
});

test('startIcon: an icon by name in a 32 px tinted square before the text, not read', async () => {
  const screen = await renderPage(`
    import { Item } from '@liulidesign/core/Item';
    export function render() { return <page><Item label="Inputs" description="Fields to type in" startIcon="text-input" hasChevron onClick={() => {}} data-testid="row" /></page>; }
  `);
  const tile = screen.getByTestId('row').children[0];
  assert.deepEqual([tile.props.width, tile.props.height, tile.props.background, tile.props.accessibilityHidden], [32, 32, 'accent-soft', 1]);
  assert.equal(tile.children[0].tag, 'vector');
});
