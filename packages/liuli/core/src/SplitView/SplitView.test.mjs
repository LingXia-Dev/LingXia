import { test } from 'node:test';
import assert from 'node:assert/strict';
import { textOf } from '@lingxia/view/testing';
import { renderExample } from '../testing/harness.mjs';

const wide = { env: { sizeClass: 'regular', viewport: { width: 1024, height: 768 } } };
const phone = { env: { sizeClass: 'compact', viewport: { width: 393, height: 852 } } };

test('Workspace: two scrolling panes, each a named group', async () => {
  const screen = await renderExample('SplitView/SplitView', 'Workspace', wide);
  const sidebar = screen.getByTestId('workspace-sidebar');
  const content = screen.getByTestId('workspace-content');
  assert.equal(sidebar.props.role, 'group');
  assert.ok(sidebar.children.some(node => node.tag === 'scroll'), 'the sidebar scrolls');
  assert.ok(content.children.some(node => node.tag === 'scroll'), 'the content scrolls');
  assert.equal(sidebar.props.accessibilityLabel, 'Mail');
  assert.equal(content.props.accessibilityLabel, 'Inbox');
});

test('Workspace: the sidebar takes its width; the content grows beside it', async () => {
  const screen = await renderExample('SplitView/SplitView', 'Workspace', wide);
  assert.equal(screen.getByTestId('workspace-sidebar').props.width, 200);
  assert.equal(screen.getByTestId('workspace-content').props.grow, 1);
});

test('Workspace: choosing a place in the sidebar shows it in the content and renames the pane', async () => {
  const screen = await renderExample('SplitView/SplitView', 'Workspace', wide);
  screen.press(screen.getByTestId('workspace-nav-launch'));
  assert.equal(textOf(screen.getByTestId('workspace-title')), 'Spring launch');
  assert.equal(screen.getByTestId('workspace-content').props.accessibilityLabel, 'Spring launch');
});

test('Workspace: a new contentKey gives the content a new scroll, so the page starts at its top', async () => {
  const screen = await renderExample('SplitView/SplitView', 'Workspace', wide);
  const scroll = () => screen.getByTestId('workspace-content').children.find(node => node.tag === 'scroll');
  assert.equal(scroll().key, 'inbox');
  screen.press(screen.getByTestId('workspace-nav-sent'));
  assert.equal(scroll().key, 'sent');
});

test('Workspace, compact: the sidebar alone, at the full width, then the content under a way back', async () => {
  const screen = await renderExample('SplitView/SplitView', 'Workspace', phone);
  assert.equal(screen.queryAll(node => node.props.testId === 'workspace-content').length, 0, 'the content is not shown beside the list');
  assert.equal(screen.getByTestId('workspace-sidebar').props.width, undefined, 'the sidebar is not held to its two-pane width');
  screen.press(screen.getByTestId('workspace-nav-launch'));
  assert.equal(screen.queryAll(node => node.props.testId === 'workspace-sidebar').length, 0, 'the list gives way to the content');
  assert.equal(textOf(screen.getByTestId('workspace-title')), 'Spring launch');
  assert.equal(screen.getByTestId('workspace-back').props.accessibilityLabel, 'Mail');
  // The chevron's stroke starts on the content's edge: 16 pt in, less the button's 13 pt and the glyph's 5.33 pt inset.
  assert.equal(screen.getByTestId('workspace-back').parent.props.marginInlineStart, -2.33, 'a logical margin, so RTL mirrors it');
  screen.press(screen.getByTestId('workspace-back'));
  assert.ok(screen.getByTestId('workspace-sidebar'), 'back shows the list again');
  assert.equal(screen.getByTestId('workspace-nav-launch').props.stateSelected, 1, 'the place chosen is still the current one');
});
