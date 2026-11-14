import { test } from 'node:test';
import assert from 'node:assert/strict';
import { textOf } from '@lingxia/view/testing';
import { renderExample } from '../testing/harness.mjs';

test('Workspace: two scrolling panes, each a named group', async () => {
  const screen = await renderExample('SplitView/SplitView', 'Workspace');
  const sidebar = screen.getByTestId('workspace-sidebar');
  const content = screen.getByTestId('workspace-content');
  assert.equal(sidebar.props.role, 'group');
  assert.ok(sidebar.children.some(node => node.tag === 'scroll'), 'the sidebar scrolls');
  assert.ok(content.children.some(node => node.tag === 'scroll'), 'the content scrolls');
  assert.equal(sidebar.props.accessibilityLabel, 'Mail');
  assert.equal(content.props.accessibilityLabel, 'Inbox');
});

test('Workspace: the sidebar takes its width; the content grows beside it', async () => {
  const screen = await renderExample('SplitView/SplitView', 'Workspace');
  assert.equal(screen.getByTestId('workspace-sidebar').props.width, 200);
  assert.equal(screen.getByTestId('workspace-content').props.grow, 1);
});

test('Workspace: choosing a place in the sidebar shows it in the content and renames the pane', async () => {
  const screen = await renderExample('SplitView/SplitView', 'Workspace');
  screen.press(screen.getByTestId('workspace-nav-launch'));
  assert.equal(textOf(screen.getByTestId('workspace-title')), 'Spring launch');
  assert.equal(screen.getByTestId('workspace-content').props.accessibilityLabel, 'Spring launch');
});

test('Workspace: a new contentKey gives the content a new scroll, so the page starts at its top', async () => {
  const screen = await renderExample('SplitView/SplitView', 'Workspace');
  const scroll = () => screen.getByTestId('workspace-content').children.find(node => node.tag === 'scroll');
  assert.equal(scroll().key, 'inbox');
  screen.press(screen.getByTestId('workspace-nav-sent'));
  assert.equal(scroll().key, 'sent');
});
