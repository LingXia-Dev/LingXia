import { test } from 'node:test';
import assert from 'node:assert/strict';
import { h, describe, prepare, commit, dispatch, useVideo, reset, VideoControls } from '../src/internal.js';
import { decode } from './support/wire.mjs';
import { escapeOverlay } from '../src/runtime/overlay.js';

// The overlay writes the time as two runs, the elapsed part brighter.
const textOf = node => node.text || (node.children ?? []).map(child => child.text).join('');

const status = { readyState: 'ready', currentTime: 12, duration: 90, paused: false, ended: false, buffering: false, seeking: false,
  buffered: 4, videoWidth: 640, videoHeight: 360, playbackRate: 1, volume: 1, muted: false, quality: 'auto', qualities: [], error: null };

function setup() {
  reset();
  const commands = [];
  globalThis.__lingxiaVideoCommand = (player, op, number, text) => { commands.push([player, op, number, text]); return ''; };
  const render = () => {
    const hero = useVideo('hero');
    return h('view', {}, h('video', { player: hero, src: 'a.mp4' }), h(VideoControls, { player: hero, 'data-testid': 'bar' }));
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
  const page = () => h('view', {}, h(Inline, {}), h('text', {}, String(tick++)), h(VideoControls, { player: useVideo('hero'), 'data-testid': 'bar' }));
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
    return h('view', {}, h('video', { player: hero, src: 'a.mp4', poster: 'a.jpg', onFullscreenChange: ({ fullscreen }) => changes.push(fullscreen) }),
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
  assert.equal(textOf(by(list, 'lx-fullscreen-controls-time')), '0:12 / 1:30');
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

test('the overlay rings while a play waits, then hides after a stretch of playback on touch', () => {
  reset();
  globalThis.__lingxiaVideoCommand = () => '';
  const render = () => {
    const hero = useVideo('hero');
    return h('view', {}, h('video', { player: hero, src: 'a.mp4' }), h(VideoControls, { player: hero, overlay: true, 'data-testid': 'bar' }));
  };
  const nodes = () => { const out = decode(prepare(render, { env: { pointer: 'coarse' } })); commit(); return out; };
  const by = (list, testId) => list.find(node => node.testId === testId);
  const report = next => {
    const video = nodes().find(node => node.tag === 'video');
    dispatch(video.props.onVideoEvent, JSON.stringify({ status: { ...status, ...next }, events: [] }));
  };
  report({ paused: true, readyState: 'metadata', currentTime: 0 });
  let list = nodes();
  assert.equal(by(list, 'bar-loading'), undefined, 'a paused player waits for no frame');
  assert.ok(by(list, 'bar-start'), 'a large play in the middle before the first play');
  dispatch(by(list, 'bar-play').props.onClick);
  assert.ok(by(nodes(), 'bar-loading'), 'a play asked for rings until the first frame');
  report({ readyState: 'ready', currentTime: 1 });
  list = nodes();
  assert.equal(by(list, 'bar-loading'), undefined);
  assert.equal(by(list, 'bar-chrome').props.opacity, 1);
  report({ currentTime: 3.5 });
  list = nodes();
  assert.equal(by(list, 'bar-chrome').props.opacity, 0, 'hidden after three seconds of playback');
  assert.ok(by(list, 'bar-play'), 'still in the tree for a screen reader');
  dispatch(by(list, 'bar-surface').props.onClick);
  assert.equal(by(nodes(), 'bar-chrome').props.opacity, 1, 'a tap brings it back');
  dispatch(by(nodes(), 'bar-surface').props.onClick);
  assert.equal(by(nodes(), 'bar-chrome').props.opacity, 0, 'and another puts it away');
  dispatch(by(nodes(), 'bar-surface').props.onClick);
  dispatch(by(nodes(), 'bar-scrub').props.onSliderChange, 60);
  report({ currentTime: 60 });
  assert.equal(by(nodes(), 'bar-chrome').props.opacity, 1, 'a seek far ahead keeps it up');
  dispatch(by(nodes(), 'bar-scrub').props.onSliderChange, 20);
  report({ currentTime: 20 });
  assert.equal(by(nodes(), 'bar-chrome').props.opacity, 1, 'and so does one back');
  report({ currentTime: 23.5 });
  assert.equal(by(nodes(), 'bar-chrome').props.opacity, 0, 'the stretch counts from where it landed');
  report({ paused: true });
  assert.equal(by(nodes(), 'bar-chrome').props.opacity, 1, 'a paused player shows it');
  delete globalThis.__lingxiaVideoCommand;
});

test('the overlay menu picks a speed or a quality and closes, and a tap beside it closes it too', () => {
  reset();
  const commands = [];
  globalThis.__lingxiaVideoCommand = (player, op, number, text) => { commands.push([op, number, text]); return ''; };
  const render = () => {
    const hero = useVideo('hero');
    return h('view', {}, h('video', { player: hero, src: 'a.mp4' }),
      h(VideoControls, { player: hero, overlay: true, labels: { settings: 'Réglages' }, 'data-testid': 'bar' }));
  };
  const nodes = () => { const out = decode(prepare(render, { env: { pointer: 'coarse' } })); commit(); return out; };
  const by = (list, testId) => list.find(node => node.testId === testId);
  const video = nodes().find(node => node.tag === 'video');
  dispatch(video.props.onVideoEvent, JSON.stringify({ status: { ...status, qualities: ['720p', '360p'] }, events: [] }));
  let list = nodes();
  assert.equal(by(list, 'bar-menu'), undefined);
  assert.equal(by(list, 'bar-settings').props.accessibilityLabel, 'Réglages');
  dispatch(by(list, 'bar-settings').props.onClick);
  list = nodes();
  assert.ok(by(list, 'bar-menu'), 'the settings button opens the menu');
  assert.equal(by(list, 'bar-chrome').props.opacity, 1, 'an open menu keeps the bar up');
  assert.ok(by(list, 'bar-rate-1') && by(list, 'bar-quality-720p') && by(list, 'bar-quality-auto'));
  dispatch(by(list, 'bar-rate-1.5').props.onClick);
  assert.deepEqual(commands.at(-1).slice(0, 2), ['rate', 1.5]);
  assert.equal(by(nodes(), 'bar-menu'), undefined, 'a pick closes it');
  dispatch(by(nodes(), 'bar-settings').props.onClick);
  dispatch(by(nodes(), 'bar-menu-dismiss').props.onClick);
  assert.equal(by(nodes(), 'bar-menu'), undefined, 'a tap beside it closes it');
  delete globalThis.__lingxiaVideoCommand;
});

test('the overlay shows an error in the middle with Retry, and full screen has a way out and the page labels', () => {
  reset();
  const commands = [];
  globalThis.__lingxiaVideoCommand = (player, op) => { commands.push(op); return ''; };
  globalThis.__lingxiaVideoOrientation = () => '';
  const render = () => {
    const hero = useVideo('hero');
    return h('view', {}, h('video', { player: hero, src: 'a.mp4' }),
      h(VideoControls, { player: hero, overlay: true, labels: { exitFullscreen: 'Quitter' }, 'data-testid': 'bar' }));
  };
  const nodes = () => { const out = decode(prepare(render, { env: { pointer: 'coarse' } })); commit(); return out; };
  const by = (list, testId) => list.find(node => node.testId === testId);
  const video = nodes().find(node => node.tag === 'video');
  dispatch(video.props.onVideoEvent, JSON.stringify({ status: { ...status, error: { code: 'VIDEO_NETWORK', message: 'VIDEO_NETWORK: x', recoverable: true } }, events: [] }));
  let list = nodes();
  assert.ok(by(list, 'bar-error'));
  assert.equal(by(list, 'bar-chrome').props.opacity, 1, 'an error keeps the bar up');
  dispatch(by(list, 'bar-retry').props.onClick);
  assert.equal(commands.at(-1), 'retry');
  dispatch(by(list, 'bar-fullscreen').props.onClick);
  list = nodes();
  const close = by(list, 'lx-fullscreen-controls-close');
  assert.equal(close.props.accessibilityLabel, 'Quitter', 'full screen speaks the page labels');
  dispatch(close.props.onClick);
  assert.equal(by(nodes(), 'lx-fullscreen-video'), undefined);
  delete globalThis.__lingxiaVideoCommand;
  delete globalThis.__lingxiaVideoOrientation;
});
