import { test } from 'node:test';
import assert from 'node:assert/strict';
import { h, List, describe, dispatch, useVideo, reset } from '../src/internal.js';
import { decode } from './support/wire.mjs';
import { escapeOverlay } from '../src/runtime/overlay.js';

const status = { readyState: 'ready', currentTime: 12, duration: 90, paused: false, ended: false, buffering: false, seeking: false,
  buffered: 4, videoWidth: 640, videoHeight: 360, playbackRate: 1, volume: 1, muted: false, quality: 'auto', qualities: [], error: null };
const by = (list, testId) => list.find(node => node.testId === testId);

function setup(draw) {
  reset();
  globalThis.__lingxiaVideoCommand = () => '';
  const turns = [];
  globalThis.__lingxiaVideoOrientation = orientation => { turns.push(orientation); return ''; };
  const changes = [];
  let hero, read;
  const page = () => {
    hero = useVideo('hero');
    read = hero.fullscreen;
    return h('view', {},
      h('video', { player: hero, src: 'a.mp4', poster: 'a.jpg', onFullscreenChange: ({ fullscreen }) => changes.push(fullscreen) }),
      h('button', { 'data-testid': 'enter', onClick: () => hero.requestFullscreen() }),
      draw && read && draw(hero));
  };
  const nodes = () => decode(describe(page, {}));
  const report = next => {
    const video = nodes().find(node => node.tag === 'video');
    assert.equal(dispatch(video.props.onVideoEvent, JSON.stringify({ status: { ...status, ...next }, events: [] })), true);
  };
  const done = () => { delete globalThis.__lingxiaVideoCommand; delete globalThis.__lingxiaVideoOrientation; };
  return { nodes, report, turns, changes, read: () => read, hero: () => hero, done };
}

test('a page that draws no full screen view gets the bare picture on black, which back leaves', () => {
  const { nodes, report, turns, changes, read, done } = setup();
  report({ videoWidth: 0, videoHeight: 0 });
  assert.equal(by(nodes(), 'lx-fullscreen-video'), undefined);
  dispatch(by(nodes(), 'enter').props.onClick);
  const list = nodes();
  assert.equal(read(), true);
  assert.ok(by(list, 'lx-fullscreen-video'));
  // The same player again, with no controls of the runtime's own.
  assert.deepEqual(list.filter(node => node.tag === 'video').map(node => node.props.videoPlayer), ['name:hero', 'name:hero']);
  assert.equal(list.filter(node => node.tag === 'button').length, 2, 'the page button and the dialog backdrop only');
  assert.deepEqual(turns, ['landscape']);
  assert.equal(escapeOverlay(), true);
  assert.equal(by(nodes(), 'lx-fullscreen-video'), undefined);
  assert.equal(read(), false);
  assert.deepEqual(changes, [true, false]);
  assert.deepEqual(turns, ['landscape', '']);
  done();
});

test('a page that draws its own full screen dialog with the player gets no second one', () => {
  const draw = hero => h('dialog', { open: true, closedby: 'closerequest', onClose: () => hero.exitFullscreen(), 'data-testid': 'mine', 'aria-label': 'Hero' },
    h('video', { player: hero }),
    h('button', { 'data-testid': 'leave', onClick: () => hero.exitFullscreen() }));
  const { nodes, read, changes, done } = setup(draw);
  dispatch(by(nodes(), 'enter').props.onClick);
  let list = nodes();
  assert.equal(read(), true);
  assert.ok(by(list, 'mine'));
  assert.equal(by(list, 'lx-fullscreen-video'), undefined);
  assert.equal(list.filter(node => node.tag === 'video').length, 2);
  dispatch(by(list, 'leave').props.onClick);
  list = nodes();
  assert.equal(read(), false);
  assert.equal(by(list, 'mine'), undefined);
  assert.deepEqual(changes, [true, false]);
  done();
});

test('a dialog that shows another player does not stand in for this one', () => {
  const draw = () => h('dialog', { open: true, 'data-testid': 'other', 'aria-label': 'Side' }, h('video', { player: useVideo('side'), src: 'b.mp4' }));
  const { nodes, done } = setup(draw);
  dispatch(by(nodes(), 'enter').props.onClick);
  const list = nodes();
  assert.ok(by(list, 'other'));
  assert.ok(by(list, 'lx-fullscreen-video'));
  done();
});

test('a list row that draws the full screen dialog gets no second one', () => {
  reset();
  globalThis.__lingxiaVideoCommand = () => '';
  globalThis.__lingxiaVideoOrientation = () => '';
  const Row = () => {
    const clip = useVideo('clip');
    return h('view', {},
      h('video', { player: clip, src: 'a.mp4' }),
      h('button', { 'data-testid': 'enter', onClick: () => clip.requestFullscreen() }),
      clip.fullscreen && h('dialog', { open: true, closedby: 'closerequest', onClose: () => clip.exitFullscreen(), 'aria-label': 'Clip', 'data-testid': 'row-full' },
        h('video', { player: clip })));
  };
  const page = () => h(List, { items: [{ id: 'a' }], getKey: item => item.id, renderItem: () => h(Row, {}) });
  const nodes = () => decode(describe(page, {}));
  dispatch(by(nodes(), 'enter').props.onClick);
  const list = nodes();
  assert.ok(by(list, 'row-full'));
  assert.equal(by(list, 'lx-fullscreen-video'), undefined);
  assert.equal(list.filter(node => node.tag === 'video').length, 2);
  delete globalThis.__lingxiaVideoCommand;
  delete globalThis.__lingxiaVideoOrientation;
});
