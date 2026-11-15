import { test } from 'node:test';
import assert from 'node:assert/strict';
import { textOf } from '@lingxia/view/testing';
import { renderExample } from '../testing/harness.mjs';

const tokens = screen => screen.queryAll(node => node.testId?.startsWith('labels-token-') && !node.testId.endsWith('-remove')).map(textOf);

test('Labels: the chosen options are tokens in a group named by the label; a token\'s button removes it', async () => {
  const screen = await renderExample('MultiSelect/MultiSelect', 'Labels');
  const box = screen.getByTestId('labels');
  assert.deepEqual([box.props.role, box.props.accessibilityLabel], ['group', 'Labels']);
  assert.deepEqual(tokens(screen), ['Travel', 'Recipes']);
  const remove = screen.getByTestId('labels-token-travel-remove');
  assert.equal(remove.props.accessibilityLabel, 'Remove Travel');
  screen.press(remove);
  assert.deepEqual(tokens(screen), ['Recipes']);
});

test('Labels: the list checks options, stays open, and keeps the options\' order', async () => {
  const screen = await renderExample('MultiSelect/MultiSelect', 'Labels');
  const add = screen.getByTestId('labels-add');
  assert.equal(add.props.accessibilityLabel, 'Add a label');
  screen.press(add);
  const item = id => screen.getByTestId(`labels-${id}`);
  assert.deepEqual([item('travel').props.role, item('travel').props.stateChecked, item('archive').props.enabled], ['menuitemcheckbox', 1, 0]);
  screen.press(item('work'));
  assert.equal(screen.queryAll(node => node.props.role === 'menu').length, 1, 'still open');
  assert.deepEqual(tokens(screen), ['Travel', 'Work', 'Recipes']);
});

test('Empty: a placeholder holds the place, and the error colours the boundary', async () => {
  const box = (await renderExample('MultiSelect/MultiSelect', 'Empty')).getByTestId('teams');
  assert.deepEqual([textOf(box.children[0]), box.props.borderColor], ['No teams yet', 'danger']);
});
