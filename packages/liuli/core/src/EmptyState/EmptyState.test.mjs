import { test } from 'node:test';
import assert from 'node:assert/strict';
import { textOf } from '@lingxia/view/testing';
import { renderExample } from '../testing/harness.mjs';

const headings = screen => screen.queryAll(node => node.props.role === 'heading');

test('FirstRun: a heading, a description, a hidden picture and the action that starts it', async () => {
  const screen = await renderExample('EmptyState/EmptyState', 'FirstRun');
  const [title] = headings(screen);
  assert.equal(textOf(title), 'No notebooks yet');
  assert.equal(title.props.headingLevel, 2);
  const root = screen.getByTestId('first-run');
  const media = root.children[0];
  assert.equal(media.props.accessibilityHidden, 1);
  assert.equal(root.props.background, undefined, 'no fill of its own');
  assert.equal(root.props.borderWidth, undefined, 'no border of its own');
  screen.press(screen.getByTestId('create'));
  assert.equal(textOf(headings(screen)[0]), 'Notebook created');
});

test('NoResults: two actions, the first clears the search', async () => {
  const screen = await renderExample('EmptyState/EmptyState', 'NoResults');
  const buttons = screen.queryAll(node => node.tag === 'button');
  assert.deepEqual(buttons.map(node => node.props.accessibilityLabel), ['Clear search', 'Search all folders']);
  screen.getByText('Nothing matches "invoice 2019". Check the spelling, or search all folders.');
  screen.press(buttons[0]);
  screen.getByText('Nothing matches "". Check the spelling, or search all folders.');
});

test('InCard: the small size takes the heading level it is given, without a picture', async () => {
  const screen = await renderExample('EmptyState/EmptyState', 'InCard');
  const levels = headings(screen).map(node => node.props.headingLevel);
  assert.deepEqual(levels, [3, 4]);
  const root = screen.getByTestId('in-card');
  assert.equal(root.children.length, 1, 'text only: no media, no actions');
});

test('sizes: the small one is tighter and its text smaller', async () => {
  const screen = await renderExample('EmptyState/EmptyState', 'Sizes');
  const [small, medium] = headings(screen);
  assert.ok(small.props.fontSize < medium.props.fontSize);
});
