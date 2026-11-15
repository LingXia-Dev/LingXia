import { test } from 'node:test';
import assert from 'node:assert/strict';
import { renderExample } from '../testing/harness.mjs';

const rows = screen => screen.queryAll(node => node.props.role === 'treeitem');

test('Folders: tree items by level; open branches show their children, closed ones do not', async () => {
  const screen = await renderExample('TreeList/TreeList', 'Folders');
  const tree = screen.getByTestId('folders');
  assert.deepEqual([tree.props.role, tree.props.accessibilityLabel], ['tree', 'Folders']);
  assert.deepEqual(rows(screen).map(row => [row.testId.slice('folders-'.length), row.props.headingLevel, row.props.stateExpanded]), [
    ['notes', 1, 1], ['travel', 2, 1], ['coast', 3, undefined], ['lisbon', 3, undefined], ['recipes', 2, 0], ['reading', 2, undefined], ['shared', 1, 0], ['archive', 1, undefined],
  ]);
  const coast = screen.getByTestId('folders-coast');
  assert.deepEqual([coast.props.stateSelected, coast.props.background], [1, 'accent-soft']);
  assert.equal(screen.getByTestId('folders-notes').props.accessibilityLabel, 'Notes, 48');
});

test('Folders: pressing a branch opens it and selects it; each level steps in', async () => {
  const screen = await renderExample('TreeList/TreeList', 'Folders');
  screen.press(screen.getByTestId('folders-recipes'));
  assert.ok(rows(screen).some(row => row.testId === 'folders-bread'));
  screen.getByText('Current: recipes');
  const inset = id => screen.getByTestId(`folders-${id}`).props.paddingInlineStart ?? screen.getByTestId(`folders-${id}`).props.paddingLeft;
  assert.deepEqual([inset('notes'), inset('recipes'), inset('bread')], [8, 28, 48]);
  screen.press(screen.getByTestId('folders-notes'));
  assert.deepEqual(rows(screen).map(row => row.testId), ['folders-notes', 'folders-shared', 'folders-archive']);
});
