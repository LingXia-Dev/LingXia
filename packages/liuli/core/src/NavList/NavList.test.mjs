import { test } from 'node:test';
import assert from 'node:assert/strict';
import { textOf } from '@lingxia/view/testing';
import { described, renderExample } from '../testing/harness.mjs';

test('Sidebar: one named tree; rows are treeitems at their level, the current one selected', async () => {
  const screen = await renderExample('NavList/NavList', 'Sidebar');
  const tree = screen.getByTestId('workspace');
  assert.equal(tree.props.role, 'tree');
  assert.equal(tree.props.accessibilityLabel, 'Workspace');
  const inbox = screen.getByTestId('workspace-inbox');
  assert.equal(inbox.tag, 'button');
  assert.equal(inbox.props.role, 'treeitem');
  assert.equal(inbox.props.stateSelected, 1);
  assert.equal(screen.getByTestId('workspace-launch').props.stateSelected, 0);
  assert.equal(screen.getByTestId('workspace-sent').props.ariaLevel, 1);
  assert.equal(screen.getByTestId('workspace-launch').props.ariaLevel, 2);
  assert.equal(screen.getByTestId('workspace-section-projects').props.ariaLevel, 1);
});

test('Sidebar: a row with a count is named by its label, the count read after it as its description', async () => {
  const screen = await renderExample('NavList/NavList', 'Sidebar');
  const inbox = screen.getByTestId('workspace-inbox');
  assert.equal(inbox.props.accessibilityLabel, 'Inbox');
  assert.deepEqual(described(screen, inbox), ['12']);
  assert.equal(screen.getByTestId('workspace-sent').props.accessibilityLabel, undefined);
});

test('Sidebar: a press makes its row current and runs onChange with its value', async () => {
  const screen = await renderExample('NavList/NavList', 'Sidebar');
  screen.press(screen.getByTestId('workspace-launch'));
  assert.equal(screen.getByTestId('workspace-launch').props.stateSelected, 1);
  assert.equal(screen.getByTestId('workspace-inbox').props.stateSelected, 0);
  assert.equal(textOf(screen.getByTestId('workspace-current')), 'Showing launch');
});

test('Sidebar: a section heading is an expanded treeitem that closes and reopens its rows', async () => {
  const screen = await renderExample('NavList/NavList', 'Sidebar');
  const heading = () => screen.getByTestId('workspace-section-projects');
  assert.equal(heading().props.role, 'treeitem');
  assert.equal(heading().props.stateExpanded, 1);
  screen.press(heading());
  assert.equal(heading().props.stateExpanded, 0);
  assert.equal(screen.queryAll(node => node.testId === 'workspace-launch').length, 0);
  screen.press(heading());
  screen.getByTestId('workspace-launch');
});

test('Sidebar: a disabled row takes no press', async () => {
  const screen = await renderExample('NavList/NavList', 'Sidebar');
  const archive = screen.getByTestId('workspace-archive');
  assert.equal(archive.props.enabled, 0);
  assert.equal(archive.props.onClick, undefined);
});

test('Closed: a section left out of defaultOpenSections starts closed and opens on a press', async () => {
  const screen = await renderExample('NavList/NavList', 'Closed');
  assert.equal(screen.getByTestId('library-section-feedback').props.stateExpanded, 0);
  assert.equal(screen.queryAll(node => node.testId === 'library-badge').length, 0);
  screen.press(screen.getByTestId('library-section-feedback'));
  screen.getByTestId('library-badge');
});

test('Sidebar: the current row is brought into view by a mark that is new for each current row, and absent when its section is closed', async () => {
  const screen = await renderExample('NavList/NavList', 'Sidebar');
  const mark = () => screen.queryAll(node => node.testId === 'workspace-reveal');
  assert.equal(mark().length, 1);
  assert.deepEqual([mark()[0].props.accessibilityHidden, mark()[0].props.position, mark()[0].props.width], [1, 'absolute', 1], 'unseen and out of the rows');
  assert.ok(mark()[0].props.onLayout, 'its layout is the moment to reveal the row');
  const first = mark()[0].key;
  assert.equal(screen.getByTestId('workspace').children.at(-1), mark()[0], 'after the rows, so the first row stays the first child');
  assert.ok(screen.getByTestId('workspace-inbox').props.elementId, 'rows carry an id to be revealed by');
  screen.layout(mark()[0], { width: 1, height: 1 });
  screen.press(screen.getByTestId('workspace-launch'));
  assert.notEqual(mark()[0].key, first, 'another row is current: a new mark, laid out afresh');
  screen.press(screen.getByTestId('workspace-section-projects'));
  assert.equal(mark().length, 0, 'the current row is in a closed section');
});
