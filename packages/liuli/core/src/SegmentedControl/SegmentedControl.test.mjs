import { test } from 'node:test';
import assert from 'node:assert/strict';
import { textOf } from '@lingxia/view/testing';
import { renderExample } from '../testing/harness.mjs';

const thumb = screen => screen.queryAll(node => node.props.widthPercent !== undefined)[0];

test('Units: a named radio group; a tap checks a segment, moves the thumb and writes the cell', async () => {
  const screen = await renderExample('SegmentedControl/SegmentedControl', 'Units');
  const group = screen.getByTestId('units');
  assert.equal(group.props.role, 'radiogroup');
  assert.equal(group.props.accessibilityLabel, 'Units');
  const km = screen.getByTestId('units-km');
  assert.equal(km.tag, 'button');
  assert.equal(km.props.role, 'radio');
  assert.equal(km.props.accessibilityLabel, 'Kilometres');
  assert.equal(km.props.stateChecked, 1);
  assert.equal(Math.round(thumb(screen).props.widthPercent), 50);
  assert.deepEqual(JSON.parse(thumb(screen).props.transform).translate, ['0%', 0]);
  screen.press(screen.getByTestId('units-mi'));
  assert.equal(screen.getByTestId('units-mi').props.stateChecked, 1);
  assert.equal(screen.getByTestId('units-km').props.stateChecked, 0);
  assert.deepEqual(JSON.parse(thumb(screen).props.transform).translate, ['100%', 0]);
  assert.equal(textOf(screen.getByTestId('distance')), '7.7 mi');
});

test('Appearance: value and onChange drive it; the thumb is a third wide', async () => {
  const screen = await renderExample('SegmentedControl/SegmentedControl', 'Appearance');
  assert.equal(screen.getByTestId('appearance-system').props.stateChecked, 1);
  screen.press(screen.getByTestId('appearance-dark'));
  assert.equal(screen.getByTestId('appearance-dark').props.stateChecked, 1);
  assert.equal(Math.round(thumb(screen).props.widthPercent), 33);
  assert.deepEqual(JSON.parse(thumb(screen).props.transform).translate, ['200%', 0]);
});

test('StateDisabled: a disabled segment stays in the row but cannot be chosen', async () => {
  const screen = await renderExample('SegmentedControl/SegmentedControl', 'StateDisabled');
  const max = screen.getByTestId('quality-max');
  assert.equal(max.props.enabled, 0);
  assert.equal(max.props.onClick, undefined);
});
