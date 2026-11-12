import { test } from 'node:test';
import assert from 'node:assert/strict';
import { described, renderExample } from '../testing/harness.mjs';

test('Volume: named by its label, described by the value in words, and the value follows the slider', async () => {
  const screen = await renderExample('Slider/Slider', 'Volume');
  const slider = screen.getByTestId('volume');
  assert.equal(slider.tag, 'slider');
  assert.equal(slider.props.accessibilityLabel, 'Volume');
  assert.deepEqual(described(screen, slider), ['40%']);
  screen.slide(slider, 72);
  assert.deepEqual(described(screen, screen.getByTestId('volume')), ['72%']);
});

test('Stepped: snaps to whole steps and reads the step name, then the help text', async () => {
  const screen = await renderExample('Slider/Slider', 'Stepped');
  const slider = screen.getByTestId('text-size');
  screen.slide(slider, 4.4);
  assert.deepEqual(described(screen, screen.getByTestId('text-size')), ['Large', 'Applies to notes and the reader.']);
});

test('Position: a hidden label stays the name, the visible time is the description', async () => {
  const screen = await renderExample('Slider/Slider', 'Position');
  const slider = screen.getByTestId('position');
  assert.equal(slider.props.accessibilityLabel, 'Position');
  assert.deepEqual(described(screen, slider), ['1:24 of 3:10']);
});

test('InList: the row variant takes the list row label style and keeps the value as the description', async () => {
  const screen = await renderExample('Slider/Slider', 'InList');
  const slider = screen.getByTestId('speed');
  assert.equal(slider.props.accessibilityLabel, 'Speed');
  assert.deepEqual(described(screen, slider), ['Normal']);
});
