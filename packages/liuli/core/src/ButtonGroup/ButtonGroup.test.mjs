import { test } from 'node:test';
import assert from 'node:assert/strict';
import { textOf } from '@lingxia/view/testing';
import { renderExample } from '../testing/harness.mjs';

test('Actions: a toolbar named by its label; the buttons keep a gap, their own variants and their presses', async () => {
  const screen = await renderExample('ButtonGroup/ButtonGroup', 'Actions');
  const group = screen.getByTestId('actions');
  assert.equal(group.props.role, 'toolbar');
  assert.equal(group.props.accessibilityLabel, 'Draft actions');
  assert.equal(group.props.gap, 8);
  assert.deepEqual(group.children.map(button => button.props.accessibilityLabel), ['Discard', 'Save', 'Publish']);
  assert.equal(screen.getByTestId('publish').props.background, 'accent-solid');
  assert.equal(screen.getByTestId('save').props.radius, 10, 'apart, each button keeps its own corners');
  screen.press(screen.getByTestId('publish'));
  assert.equal(textOf(screen.getByTestId('outcome')), 'Published');
});

test('Attached: no gap, shared edges, and only the group\'s two ends are round', async () => {
  const screen = await renderExample('ButtonGroup/ButtonGroup', 'Attached');
  assert.equal(screen.getByTestId('history').props.columnGap, 0);
  const [undo, redo, reset] = ['undo', 'redo', 'reset'].map(id => screen.getByTestId(id));
  const round = button => JSON.stringify(['radiusStartStart', 'radiusStartEnd', 'radiusEndEnd', 'radiusEndStart'].map(corner => button.props[corner] ?? 0));
  assert.equal(round(redo), '[0,0,0,0]', 'a middle button is square');
  assert.equal(round(undo), '[10,0,0,10]', 'the first rounds its start edge');
  assert.equal(round(reset), '[0,10,10,0]', 'the last rounds its end edge');
  assert.notEqual(round(undo), '[0,0,0,0]');
  assert.notEqual(round(reset), '[0,0,0,0]');
  assert.notEqual(round(undo), round(reset), 'the first rounds its start edge, the last its end edge');
  assert.equal(redo.props.marginInlineStart, -1, 'neighbours overlap by their edge, so it is drawn once');
  assert.equal(undo.props.marginInlineStart ?? 0, 0);
  assert.ok(undo.children.some(child => child.tag === 'vector'), 'a start icon by name');
  assert.match(redo.props.focusVisibleStyle.ring, /"inner":\{"width":2,"color":"focus"\}/, 'the focus ring is inside the button, where a neighbour cannot cover it');
  assert.doesNotMatch(redo.props.focusVisibleStyle.ring, /outer/);
});

test('the group gives its buttons a variant, a size and a disabled state; a button that sets its own keeps it', async () => {
  const icons = await renderExample('ButtonGroup/ButtonGroup', 'IconsAttached');
  const out = icons.getByTestId('zoom-out');
  assert.deepEqual([out.props.background, out.props.width, out.props.accessibilityLabel], ['surface', 40, 'Zoom out']);
  const sizes = await renderExample('ButtonGroup/ButtonGroup', 'Sizes');
  assert.deepEqual([sizes.getByTestId('small-day').props.minHeight, sizes.getByTestId('large-day').props.minHeight], [32, 48]);
  const disabled = await renderExample('ButtonGroup/ButtonGroup', 'StateDisabled');
  assert.equal(disabled.getByTestId('disabled-undo').props.onClick, undefined, 'the group disables its buttons');
  const actions = await renderExample('ButtonGroup/ButtonGroup', 'Actions');
  assert.equal(actions.getByTestId('discard').props.background, '#00000000', 'its own ghost variant');
});

test('Toolbar: icon buttons in a quiet group, each named', async () => {
  const screen = await renderExample('ButtonGroup/ButtonGroup', 'Toolbar');
  const group = screen.getByTestId('message');
  assert.deepEqual(group.children.map(button => [button.props.accessibilityLabel, button.props.width, button.props.background]), [
    ['Reply', 32, '#00000000'], ['Forward', 32, '#00000000'], ['Delete', 32, '#00000000'],
  ]);
});

test('An attached group too long for one line comes apart, each button rounded with a gap, and joins again once it fits', async () => {
  const screen = await renderExample('ButtonGroup/ButtonGroup', 'Attached');
  const group = () => screen.getByTestId('history');
  const round = id => ['radiusStartStart', 'radiusStartEnd', 'radiusEndEnd', 'radiusEndStart'].map(corner => screen.getByTestId(id).props[corner] ?? screen.getByTestId(id).props.radius ?? 0);
  // Joined, three buttons need 300 pt on one line; the card gives 200.
  screen.layout(group(), { width: 200, height: 92, minContentWidth: 100, maxContentWidth: 300 });
  assert.equal(group().props.columnGap ?? group().props.gap, 8, 'apart: a gap between buttons');
  assert.deepEqual(round('redo'), [10, 10, 10, 10], 'the middle button is whole');
  assert.equal(screen.getByTestId('redo').props.marginInlineStart ?? 0, 0, 'no overlap');
  // Apart its one line is 300 + two 9 pt joins; at 310 joined it fits (300), apart it would not (318).
  screen.layout(group(), { width: 310, height: 92, minContentWidth: 100, maxContentWidth: 318 });
  assert.equal(group().props.columnGap, 0, 'joined again');
  assert.deepEqual(round('redo'), [0, 0, 0, 0]);
  // Joined and fitting, it stays joined.
  screen.layout(group(), { width: 300, height: 40, minContentWidth: 100, maxContentWidth: 300 });
  assert.equal(group().props.columnGap, 0);
});

test('At large text: a group wraps rather than run past its card, attached or apart, and its buttons keep their labels whole', async () => {
  for (const [example, id, gaps] of [['Attached', 'history', [0, 8]], ['Actions', 'actions', [8, 8]]]) {
    const screen = await renderExample('ButtonGroup/ButtonGroup', example);
    const group = screen.getByTestId(id);
    assert.deepEqual([group.props.flexWrap, group.props.maxWidthPercent, group.props.columnGap ?? group.props.gap, group.props.rowGap ?? group.props.gap], ['wrap', 100, ...gaps], example);
    assert.ok(group.children.every(button => button.props.shrink === 0), example);
  }
  const toolbar = await renderExample('ButtonGroup/ButtonGroup', 'Toolbar');
  assert.equal(toolbar.queryAll(node => node.children.includes(toolbar.getByTestId('message')))[0].props.flexWrap, 'wrap', 'the group and the action beside it wrap too');
});
