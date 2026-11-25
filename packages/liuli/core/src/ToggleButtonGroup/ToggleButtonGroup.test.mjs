import { test } from 'node:test';
import assert from 'node:assert/strict';
import { textOf } from '@lingxia/view/testing';
import { renderExample } from '../testing/harness.mjs';

test('Filters: a toolbar of toggle buttons, each pressed on its own, in the items\' order', async () => {
  const screen = await renderExample('ToggleButtonGroup/ToggleButtonGroup', 'Filters');
  const group = screen.getByTestId('filters');
  assert.deepEqual([group.props.role, group.props.accessibilityLabel], ['toolbar', 'Filters']);
  const pressed = () => ['pinned', 'starred', 'files'].map(value => screen.getByTestId(`filters-${value}`).props.statePressed);
  assert.deepEqual(pressed(), [0, 1, 0]);
  assert.equal(textOf(screen.getByTestId('filters-count')), '2 notes');
  screen.press(screen.getByTestId('filters-files'));
  assert.deepEqual(pressed(), [0, 1, 1]);
  assert.equal(textOf(screen.getByTestId('filters-count')), '1 note');
  screen.press(screen.getByTestId('filters-starred'));
  assert.deepEqual(pressed(), [0, 0, 1]);
  const on = screen.getByTestId('filters-files');
  assert.deepEqual([on.props.background, on.props.borderColor], ['accent-soft', 'accent'], 'on is the fill and the edge');
  assert.equal(on.props.accessibilityLabel, 'With files');
});

test('Layout: one on at a time is a radio group; icon-only toggles are named by their labels', async () => {
  const screen = await renderExample('ToggleButtonGroup/ToggleButtonGroup', 'Layout');
  assert.equal(screen.getByTestId('layout').props.role, 'radiogroup');
  const grid = screen.getByTestId('layout-grid');
  assert.deepEqual([grid.props.role, grid.props.accessibilityLabel, grid.props.stateChecked, grid.props.statePressed], ['radio', 'Grid', 1, undefined]);
  assert.equal(JSON.stringify(grid.children).includes('Grid'), false, 'the label is not drawn');
  screen.press(screen.getByTestId('layout-list'));
  assert.equal(screen.getByTestId('layout-list').props.stateChecked, 1);
  assert.equal(screen.getByTestId('layout-grid').props.stateChecked, 0);
  screen.press(screen.getByTestId('layout-list'));
  assert.equal(screen.getByTestId('layout-list').props.stateChecked, 1, 'pressing the one on keeps it on');
  assert.equal(textOf(screen.getByTestId('layout-shown')), 'Showing notes as a list');
});

test('StateDisabled: every toggle is disabled and keeps its state', async () => {
  const screen = await renderExample('ToggleButtonGroup/ToggleButtonGroup', 'StateDisabled');
  const buttons = screen.queryAll(node => node.tag === 'button');
  assert.deepEqual(buttons.map(button => [button.props.enabled, button.props.statePressed]), [[0, 0], [0, 1]]);
});
