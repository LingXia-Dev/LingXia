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
