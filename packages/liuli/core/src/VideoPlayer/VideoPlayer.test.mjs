import { test } from 'node:test';
import assert from 'node:assert/strict';
import { renderExample } from '../testing/harness.mjs';

test('Standard: the video is named, its controls follow it, and a large play waits for the first play', async () => {
  const screen = await renderExample('VideoPlayer/VideoPlayer', 'Standard');
  const video = screen.getByTestId('standard');
  assert.equal(video.tag, 'video');
  assert.equal(video.props.accessibilityLabel, 'Big Buck Bunny');
  assert.ok(screen.getByTestId('standard-controls-start'));
  assert.equal(screen.getByTestId('standard-controls-scrub').props.accessibilityLabel, 'Seek');
});

test('UnderThePicture: the controls sit in a row under the picture', async () => {
  const screen = await renderExample('VideoPlayer/VideoPlayer', 'UnderThePicture');
  assert.equal(screen.getByTestId('under').tag, 'video');
  assert.equal(screen.queryAll(node => node.props.testId === 'under-controls-start').length, 0);
  assert.equal(screen.getByTestId('under-controls-play').props.accessibilityLabel, 'Play');
});

test('UnderThePicture: the poster reaches the picture and the full-screen picture', async () => {
  const screen = await renderExample('VideoPlayer/VideoPlayer', 'UnderThePicture');
  const poster = screen.getByTestId('under').props.videoPoster;
  assert.match(poster, /\.jpg$/);
  screen.press(screen.getByTestId('under-controls-fullscreen'));
  const pictures = screen.queryAll(node => node.tag === 'video' && node.props.videoPoster === poster);
  assert.equal(pictures.length, 2);
});
