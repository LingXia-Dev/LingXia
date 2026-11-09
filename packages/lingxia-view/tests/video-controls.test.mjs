import { test } from 'node:test';
import assert from 'node:assert/strict';
import { h, describe, dispatch, useVideo, reset, VideoControls } from '../src/internal.js';
import { decode } from './support/wire.mjs';
import { escapeOverlay } from '../src/runtime/overlay.js';

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

test('the full screen view keeps the poster of a page view that did not re-render', () => {
  const { by } = setup();
  globalThis.__lingxiaVideoOrientation = () => '';
  let runs = 0;
  const Inline = () => (runs++, h('video', { player: useVideo('hero'), src: 'a.mp4', poster: 'a.jpg' }));
  // The ticking sibling commits every render, so Inline is reused rather than re-run.
  let tick = 0;
  const page = () => h('column', {}, h(Inline, {}), h('text', {}, String(tick++)), h(VideoControls, { player: useVideo('hero'), 'data-testid': 'bar' }));
  const all = () => decode(describe(page, {}));
  all();
  dispatch(by(all(), 'bar-fullscreen').props.onClick);
  all(); all();
  const views = all().filter(node => node.tag === 'video');
  assert.ok(runs < 5, 'Inline was reused');
  assert.deepEqual(views.map(node => node.props.videoPoster), ['a.jpg', 'a.jpg']);
  delete globalThis.__lingxiaVideoCommand;
  delete globalThis.__lingxiaVideoOrientation;
});

test('VideoControls full screen shows the player again in the top layer and a close request leaves it', () => {
  const { nodes, by, report } = setup();
  const turns = [];
  globalThis.__lingxiaVideoOrientation = orientation => { turns.push(orientation); return ''; };
  report({ videoWidth: 0, videoHeight: 0 });
  let read;
  const changes = [];
  const probe = () => {
    const hero = useVideo('hero');
    read = hero.fullscreen;
    return h('column', {}, h('video', { player: hero, src: 'a.mp4', poster: 'a.jpg', onFullscreenChange: ({ fullscreen }) => changes.push(fullscreen) }),
      h(VideoControls, { player: hero, 'data-testid': 'bar' }));
  };
  const all = () => decode(describe(probe, {}));
  let list = all();
  assert.equal(by(list, 'bar-fullscreen').props.accessibilityLabel, 'Full screen');
  assert.equal(by(list, 'lx-fullscreen-video'), undefined);
  dispatch(by(list, 'bar-fullscreen').props.onClick);
  list = all();
  assert.equal(read, true);
  const layer = by(list, 'lx-fullscreen-video');
  assert.ok(layer, 'a full screen view in the top layer');
  const views = list.filter(node => node.tag === 'video');
  assert.deepEqual(views.map(node => node.props.videoPlayer), ['name:hero', 'name:hero']);
  // Before its first frame the full screen view shows the page's poster.
  assert.deepEqual(views.map(node => node.props.videoPoster), ['a.jpg', 'a.jpg']);
  assert.equal(by(list, 'lx-fullscreen-controls-fullscreen').props.accessibilityLabel, 'Exit full screen');
  // Landscape before the picture's shape is known; a wide one keeps it.
  assert.deepEqual(turns, ['landscape']);
  report({ videoWidth: 1920, videoHeight: 1080 });
  assert.deepEqual(turns, ['landscape']);
  list = all();
  // The full screen bar starts from the player's last state.
  assert.equal(by(list, 'lx-fullscreen-controls-time').text, '0:12 / 1:30');
  dispatch(by(list, 'lx-fullscreen-controls-fullscreen').props.onClick);
  list = all();
  assert.equal(read, false);
  assert.equal(by(list, 'lx-fullscreen-video'), undefined);
  // Back (a close request) leaves full screen too.
  dispatch(by(list, 'bar-fullscreen').props.onClick);
  assert.ok(by(all(), 'lx-fullscreen-video'));
  assert.equal(escapeOverlay(), true);
  assert.equal(by(all(), 'lx-fullscreen-video'), undefined);
  assert.equal(read, false);
  assert.deepEqual(changes, [true, false, true, false]);
  // Leaving gives the window back; a tall picture asks for portrait, a square one for nothing.
  report({ videoWidth: 1080, videoHeight: 1920 });
  dispatch(by(all(), 'bar-fullscreen').props.onClick);
  report({ videoWidth: 1080, videoHeight: 1080 });
  dispatch(by(all(), 'lx-fullscreen-controls-fullscreen').props.onClick);
  assert.deepEqual(turns, ['landscape', '', 'landscape', '', 'portrait', '']);
  void nodes;
  delete globalThis.__lingxiaVideoCommand;
  delete globalThis.__lingxiaVideoOrientation;
});
