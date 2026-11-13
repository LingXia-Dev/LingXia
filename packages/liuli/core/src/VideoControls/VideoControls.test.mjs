import { test } from 'node:test';
import assert from 'node:assert/strict';
import { renderExample } from '../testing/harness.mjs';

const play = (screen, prefix) => screen.getByTestId(`${prefix}-play`);

test('Overlay: a large play before the first play, every control named, the bar in the tree', async () => {
  const screen = await renderExample('VideoControls/VideoControls', 'Overlay');
  assert.ok(screen.getByTestId('overlay-controls-start'));
  assert.equal(play(screen, 'overlay-controls').props.accessibilityLabel, 'Play');
  assert.equal(screen.getByTestId('overlay-controls-mute').props.accessibilityLabel, 'Mute');
  assert.equal(screen.getByTestId('overlay-controls-fullscreen').props.accessibilityLabel, 'Full screen');
  assert.equal(screen.getByTestId('overlay-controls-settings').props.accessibilityLabel, 'Playback settings');
});

test('Overlay: the scrubber reads its time, runs left to right, and its touch box ends where the buttons start', async () => {
  const screen = await renderExample('VideoControls/VideoControls', 'Overlay');
  const scrub = screen.getByTestId('overlay-controls-scrub');
  assert.equal(scrub.tag, 'slider');
  assert.equal(scrub.props.accessibilityLabel, 'Seek');
  assert.equal(scrub.props.ariaValueText, '0:00 / --:--');
  assert.equal(scrub.props.direction, 'ltr');
  const bar = screen.getByTestId('overlay-controls-bar');
  assert.equal(scrub.props.insetTop + scrub.props.height, bar.props.paddingTop);
  assert.ok(scrub.props.height >= 44);
  // Before metadata it is disabled; keyboard focus still shows the thumb.
  assert.equal(scrub.props.enabled, 0);
  assert.ok(scrub.props.focusVisibleStyle.thumbStyle);
});

test('Bar: a row with the time beside the slider', async () => {
  const screen = await renderExample('VideoControls/VideoControls', 'Bar');
  assert.equal(screen.getByTestId('bar-controls-time').props.text ?? null, null);
  assert.equal(screen.getByTestId('bar-controls-scrub').props.accessibilityLabel, 'Seek');
  assert.equal(play(screen, 'bar-controls').props.accessibilityLabel, 'Play');
  assert.equal(screen.getByTestId('bar-controls-settings').props.accessibilityLabel, 'Playback settings');
});

test('Bar: the slider under the picture keeps Slider\'s md look, the buffered part aside', async () => {
  const { sliderRecipe } = await import('../Slider/Slider.recipe.ts');
  const { videoControlsRecipe } = await import('./VideoControls.recipe.ts');
  // Part and state classes: thumb, range, track, pressed, hovered, focused, disabled.
  const look = classes => new Set(classes.split(' ').filter(c => /^(thumb|range|track|active|hover|focus-visible|disabled):/.test(c)));
  const slider = look(`${sliderRecipe.control.base} ${sliderRecipe.control.size.md}`);
  assert.deepEqual([...look(videoControlsRecipe.barRow.slider)].sort(), [...slider].sort());
});


const status = { readyState: 'ready', currentTime: 1, duration: 10, paused: true, ended: false, buffering: false, seeking: false,
  buffered: 2, videoWidth: 1280, videoHeight: 720, playbackRate: 1, volume: 1, muted: false, quality: 'auto', qualities: [], error: null };
const report = (screen, next) => screen.video(screen.queryAll(node => node.tag === 'video')[0], { ...status, ...next });
const shown = screen => screen.getByTestId('overlay-controls-scrub').props.ariaValueText;

test('Overlay scrubber: a finger still down keeps the dragged place after a paused seek lands, and lets go on release', async () => {
  const screen = await renderExample('VideoControls/VideoControls', 'Overlay');
  report(screen, {});
  // The slider's press (the wire's onSliderPress); the host ends it with its release call.
  screen.fire(screen.getByTestId('overlay-controls-scrub'), 'onSliderPress');
  screen.slide(screen.getByTestId('overlay-controls-scrub'), 5);
  report(screen, { currentTime: 5, seeking: true });
  report(screen, { currentTime: 5 });
  // The player wanders while the finger rests: the thumb stays where it was put.
  report(screen, { currentTime: 4 });
  assert.equal(shown(screen), '0:05 / 0:10');
  assert.equal(screen.host.__lingxiaReleasePress(), true);
  screen.rerender({});
  assert.equal(shown(screen), '0:04 / 0:10');
});

test('Overlay scrubber: without the press signal the landed seek alone ends the hold', async () => {
  const screen = await renderExample('VideoControls/VideoControls', 'Overlay');
  report(screen, {});
  screen.slide(screen.getByTestId('overlay-controls-scrub'), 5);
  assert.equal(shown(screen), '0:05 / 0:10', 'held until the player reaches it');
  report(screen, { currentTime: 5, seeking: true });
  report(screen, { currentTime: 5 });
  report(screen, { currentTime: 4 });
  assert.equal(shown(screen), '0:04 / 0:10');
});
