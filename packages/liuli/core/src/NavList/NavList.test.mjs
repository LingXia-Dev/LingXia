import { test } from 'node:test';
import assert from 'node:assert/strict';
import { textOf } from '@lingxia/view/testing';
import { renderExample } from '../testing/harness.mjs';

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
  assert.equal(screen.getByTestId('workspace-sent').props.headingLevel, 1);
  assert.equal(screen.getByTestId('workspace-launch').props.headingLevel, 2);
  assert.equal(screen.getByTestId('workspace-section-projects').props.headingLevel, 1);
});

test('Sidebar: a row with a count is named with it; the count itself is not read again', async () => {
  const screen = await renderExample('NavList/NavList', 'Sidebar');
  assert.equal(screen.getByTestId('workspace-inbox').props.accessibilityLabel, 'Inbox, 12');
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

test('Closed: a section in defaultClosed starts closed and opens on a press', async () => {
  const screen = await renderExample('NavList/NavList', 'Closed');
  assert.equal(screen.getByTestId('library-section-feedback').props.stateExpanded, 0);
  assert.equal(screen.queryAll(node => node.testId === 'library-badge').length, 0);
  screen.press(screen.getByTestId('library-section-feedback'));
  screen.getByTestId('library-badge');
});
