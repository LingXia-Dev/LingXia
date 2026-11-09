import { test } from 'node:test';
import assert from 'node:assert/strict';
import { h, describe, dispatch, useVideo, reset, VideoControls } from '../src/internal.js';
import { decode } from './support/wire.mjs';

const status = { readyState: 'ready', currentTime: 12, duration: 90, paused: false, ended: false, buffering: false, seeking: false,
  buffered: 4, videoWidth: 640, videoHeight: 360, playbackRate: 1, volume: 1, muted: false, quality: 'auto', qualities: [], error: null };

function setup() {
  reset();
  const commands = [];
  globalThis.__lingxiaVideoCommand = (player, op, number, text) => { commands.push([player, op, number, text]); return ''; };
  const render = () => {
    const hero = useVideo('hero');
    return h('column', {}, h('video', { player: hero, src: 'a.mp4' }), h(VideoControls, { player: hero, 'data-testid': 'bar' }));
  };
  const nodes = () => decode(describe(render, {}));
  const by = (list, testId) => list.find(node => node.testId === testId);
  const report = (next) => {
    const video = nodes().find(node => node.tag === 'video');
    assert.equal(dispatch(video.props.onVideoEvent, JSON.stringify({ status: { ...status, ...next }, events: [] })), true);
  };
  return { commands, nodes, by, report };
}

test('VideoControls plays, pauses, mutes and shows the time', () => {
  const { commands, nodes, by, report } = setup();
  report({});
  let list = nodes();
  assert.equal(by(list, 'bar-play').props.accessibilityLabel, 'Pause');
  assert.equal(by(list, 'bar-time').text, '0:12 / 1:30');
  assert.equal(by(list, 'bar-scrub').props.sliderValue, 12);
  assert.equal(by(list, 'bar-quality'), undefined, 'no menu without qualities');
  dispatch(by(list, 'bar-play').props.onClick);
  dispatch(by(list, 'bar-mute').props.onClick);
  assert.deepEqual(commands.map(([, op, number]) => [op, number]), [['pause', 0], ['muted', 1]]);
  report({ paused: true, muted: true, qualities: ['1080p', '720p'], quality: '720p' });
  list = nodes();
  assert.equal(by(list, 'bar-play').props.accessibilityLabel, 'Play');
  assert.equal(by(list, 'bar-mute').props.accessibilityLabel, 'Unmute');
  assert.ok(by(list, 'bar-quality'), 'a menu once the player has qualities');
  report({ currentTime: 3725, duration: 7200 });
  assert.equal(by(nodes(), 'bar-time').text, '1:02:05 / 2:00:00');
  delete globalThis.__lingxiaVideoCommand;
});

test('VideoControls holds a dragged position until the player reaches it', () => {
  const { commands, nodes, by, report } = setup();
  report({});
  dispatch(by(nodes(), 'bar-scrub').props.onSliderChange, 60);
  assert.deepEqual(commands.at(-1).slice(1, 3), ['seek', 60]);
  // The old time still arrives before the seek lands: the thumb stays where it was dropped.
  report({ currentTime: 12.3, seeking: true });
  assert.equal(by(nodes(), 'bar-scrub').props.sliderValue, 60);
  report({ currentTime: 60.1, seeking: false });
  assert.equal(by(nodes(), 'bar-scrub').props.sliderValue, 60.1);
  report({ currentTime: 61 });
  assert.equal(by(nodes(), 'bar-scrub').props.sliderValue, 61, 'released once reached');
  delete globalThis.__lingxiaVideoCommand;
});

test('VideoControls replays once ended, shows Live without a duration and offers Retry', () => {
  const { commands, nodes, by, report } = setup();
  let list = nodes();
  assert.equal(by(list, 'bar-time').text, '0:00 / --:--', 'before metadata');
  assert.equal(by(list, 'bar-scrub').props.enabled, 0);
  report({ ended: true, paused: true, currentTime: 90 });
  list = nodes();
  assert.equal(by(list, 'bar-play').props.accessibilityLabel, 'Replay');
  dispatch(by(list, 'bar-play').props.onClick);
  assert.deepEqual(commands.at(-1).slice(1, 3), ['seek', 0]);
  report({ duration: null });
  list = nodes();
  assert.ok(by(list, 'bar-live'));
  assert.equal(by(list, 'bar-scrub'), undefined);
  report({ error: { code: 'VIDEO_NETWORK', message: 'Check the connection.', recoverable: true } });
  list = nodes();
  dispatch(by(list, 'bar-retry').props.onClick);
  assert.deepEqual(commands.at(-1).slice(1, 2), ['retry']);
  assert.throws(() => describe(() => h(VideoControls, { player: {} }), {}), /UI_PROP/);
  delete globalThis.__lingxiaVideoCommand;
});
