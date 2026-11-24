import { test } from 'node:test';
import assert from 'node:assert/strict';
import { renderExample } from '../testing/harness.mjs';

const menus = screen => screen.queryAll(node => node.props.role === 'menu');

test('Rows: nothing is open until a row is asked for its menu; the row listens for the ask', async () => {
  const screen = await renderExample('ContextMenu/ContextMenu', 'Rows');
  assert.equal(menus(screen).length, 0);
  const row = screen.getByTestId('note-coast');
  assert.ok(Object.keys(row.props).some(key => /context/i.test(key)), JSON.stringify(Object.keys(row.props)));
});

const ask = async (screen, id, pointer) => {
  screen.fire(screen.getByTestId(id), 'onContextMenu', JSON.stringify({ x: 120, y: 200, pointer }));
  await new Promise(resolve => setImmediate(resolve));
};
const status = screen => screen.queryAll(node => node.props.role === 'status').map(node => node.text ?? node.children.map(child => child.text).join('')).join('');

test('Rows: from a finger the menu docks over a scrim, edge to edge alike; an item runs its action and closes it', async () => {
  const screen = await renderExample('ContextMenu/ContextMenu', 'Rows');
  await ask(screen, 'note-coast', 'touch');
  const sheet = screen.getByTestId('note-coast-menu');
  assert.notEqual(sheet.props.position, 'fixed', 'placed in its layer: a fixed box stops at the bottom safe area');
  assert.deepEqual([sheet.props.marginLeft ?? 0, sheet.props.marginRight ?? 0], [0, 0], 'the sheet is as far from one edge as from the other');
  assert.equal(screen.getByTestId('note-coast-menu-backdrop').props.background, 'scrim', 'the page is dimmed under it');
  const remove = screen.queryAll(node => node.tag === 'text' && node.text === 'Delete');
  assert.deepEqual(remove.map(node => node.props.color), ['danger'], 'a destructive item is named in the danger role');
  screen.press(screen.getByTestId('pin-coast'));
  await new Promise(resolve => setImmediate(resolve));
  assert.equal(status(screen), 'Pinned Trip to the coast', 'the action ran');
  assert.equal(menus(screen).length, 0, 'and the menu closed');
});

test('Rows: the scrim closes the docked menu and runs nothing; from a mouse the page is not dimmed', async () => {
  const screen = await renderExample('ContextMenu/ContextMenu', 'Rows');
  await ask(screen, 'note-reading', 'touch');
  screen.press(screen.getByTestId('note-reading-menu-backdrop'));
  await new Promise(resolve => setImmediate(resolve));
  assert.deepEqual([menus(screen).length, status(screen)], [0, '']);
  await ask(screen, 'note-reading', 'mouse');
  const menu = screen.getByTestId('note-reading-menu');
  assert.equal(menu.props.role, 'menu');
  assert.equal(menu.props.position, 'absolute');
  assert.notEqual(screen.getByTestId('note-reading-menu-backdrop').props.background, 'scrim');
});

test('Data: rows given as data open on the ask, with a submenu, and run their action', async () => {
  const screen = await renderExample('ContextMenu/ContextMenu', 'Data');
  await ask(screen, 'photo-dunes', 'mouse');
  const labels = screen.queryAll(node => node.props.role === 'menuitem').map(node => node.props.accessibilityLabel);
  assert.deepEqual(labels.slice(0, 3), ['Add to album', 'Share', 'Delete']);
  screen.press(screen.getByTestId('photo-delete'));
  await new Promise(resolve => setImmediate(resolve));
  assert.equal(status(screen), 'Deleted Dunes at noon');
});

test('Rows: at a point the menu is as wide as its widest item, up to 90 % of the screen, so a long label sets its width before it wraps and the menu grows taller with it', async () => {
  const screen = await renderExample('ContextMenu/ContextMenu', 'Rows');
  await ask(screen, 'note-coast', 'mouse');
  const menu = screen.getByTestId('note-coast-menu');
  assert.deepEqual([menu.props.minWidth, menu.props.maxWidth, menu.props.width, menu.props.height], [192, 351, undefined, undefined], 'at least 192 pt, at most 90 % of the 390 pt screen');
  const label = screen.queryAll(node => node.tag === 'text' && node.text === 'Pin to top')[0];
  const text = label.parent;
  assert.deepEqual([text.props.flexBasis, text.props.grow, text.props.minWidth], [undefined, 1, 0], 'the label\'s own width counts toward the menu\'s');
  assert.equal(label.parent.parent.props.height, undefined, 'an item has no fixed height: a wrapped label makes it taller');
});
