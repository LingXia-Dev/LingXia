import { test } from 'node:test';
import assert from 'node:assert/strict';
import { described, renderExample } from '../testing/harness.mjs';

test('Volume: named by its label, the words are its value, and they follow the slider', async () => {
  const screen = await renderExample('Slider/Slider', 'Volume');
  const slider = screen.getByTestId('volume');
  assert.equal(slider.tag, 'slider');
  assert.equal(slider.props.accessibilityLabel, 'Volume');
  assert.equal(slider.props.ariaValueText, '40%');
  assert.deepEqual(described(screen, slider), []);
  screen.slide(slider, 72);
  assert.equal(screen.getByTestId('volume').props.ariaValueText, '72%');
});

test('Stepped: snaps to whole steps, reads the step name as the value and the help text as the description', async () => {
  const screen = await renderExample('Slider/Slider', 'Stepped');
  const slider = screen.getByTestId('text-size');
  screen.slide(slider, 4.4);
  const moved = screen.getByTestId('text-size');
  assert.equal(moved.props.sliderValue, 4);
  assert.equal(moved.props.ariaValueText, 'Large');
  assert.deepEqual(described(screen, moved), ['Applies to notes and the reader.']);
});

test('Position: a hidden label stays the name, the visible time is the value, and the loaded part is drawn', async () => {
  const screen = await renderExample('Slider/Slider', 'Position');
  const slider = screen.getByTestId('position');
  assert.equal(slider.props.accessibilityLabel, 'Position');
  assert.equal(slider.props.ariaValueText, '1:24 of 3:10');
  assert.ok(slider.props.secondaryStyle);
});

test('InList: the row variant keeps the words as the value', async () => {
  const screen = await renderExample('Slider/Slider', 'InList');
  const slider = screen.getByTestId('speed');
  assert.equal(slider.props.accessibilityLabel, 'Speed');
  assert.equal(slider.props.ariaValueText, 'Normal');
});

test('the thumb grows while pressed and rings on keyboard focus', async () => {
  const screen = await renderExample('Slider/Slider', 'Volume');
  const slider = screen.getByTestId('volume');
  assert.notEqual(slider.props.pressedStyle.thumbStyle, slider.props.thumbStyle);
  assert.match(slider.props.focusVisibleStyle.thumbStyle, /"color\\?":\\?"focus/);
});

test('Marks: a tick at each value\'s share of the track, labels at the ends, none of it read', async () => {
  const screen = await renderExample('Slider/Slider', 'Marks');
  const marks = screen.getByTestId('marked').parent.children.find(node => node.props.accessibilityHidden === 1 && node.props.flexDirection === 'row' && node.children.length === 10);
  assert.deepEqual(marks.children.filter((_, index) => index % 2 === 0).map(gap => gap.props.widthPercent), [0, 25, 25, 25, 25]);
  const seats = marks.children.filter((_, index) => index % 2 === 1);
  assert.deepEqual(seats.map(seat => [seat.props.width, seat.props.align]), [[0, 'start'], [0, 'center'], [0, 'center'], [0, 'center'], [0, 'end']]);
});

test('SaveOnRelease: the slider listens for the finger leaving', async () => {
  const slider = (await renderExample('Slider/Slider', 'SaveOnRelease')).getByTestId('brightness');
  assert.ok(slider.props.onSliderPress);
});
