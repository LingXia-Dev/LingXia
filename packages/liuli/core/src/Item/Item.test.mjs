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
