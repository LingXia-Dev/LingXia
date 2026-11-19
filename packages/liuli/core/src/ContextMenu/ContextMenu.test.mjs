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
  const panel = sheet.parent;
  assert.equal(panel.props.position, 'absolute', 'placed in its layer: a fixed box stops at the bottom safe area');
  assert.deepEqual([panel.props.marginLeft ?? 0, panel.props.marginRight ?? 0], [0, 0], 'no margin of the panel\'s own pushes it aside');
  assert.equal(sheet.props.marginLeft, sheet.props.marginRight, 'the sheet is as far from one edge as from the other');
  assert.equal(screen.getByTestId('note-coast-scrim').props.background, 'scrim', 'the page is dimmed under it');
  const remove = screen.queryAll(node => node.tag === 'text' && node.text === 'Delete');
  assert.deepEqual(remove.map(node => node.props.color), ['danger'], 'a destructive item is named in the danger role');
  screen.press(screen.getByTestId('pin-coast'));
  await new Promise(resolve => setImmediate(resolve));
  assert.equal(status(screen), 'Pinned Trip to the coast', 'the action ran');
  assert.equal(menus(screen).length, 0, 'and the menu closed');
});

test('Rows: the scrim closes the docked menu and runs nothing; from a mouse there is no scrim', async () => {
  const screen = await renderExample('ContextMenu/ContextMenu', 'Rows');
  await ask(screen, 'note-reading', 'touch');
  screen.press(screen.getByTestId('note-reading-scrim'));
  await new Promise(resolve => setImmediate(resolve));
  assert.deepEqual([menus(screen).length, status(screen)], [0, '']);
  await ask(screen, 'note-reading', 'mouse');
  const menu = screen.getByTestId('note-reading-menu');
  assert.equal(menu.props.role, 'menu');
  assert.equal(menu.props.position, 'absolute');
  assert.equal(screen.queryAll(node => node.testId === 'note-reading-scrim').length, 0);
});
