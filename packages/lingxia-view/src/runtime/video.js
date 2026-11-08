// `useVideo(name)`: the page's player, read during render and driven from handlers.
import { local, cellOf } from './hooks.js';
import { render, page, outsideRender } from './state.js';

const fields = ['paused', 'ended', 'buffering', 'seeking', 'readyState', 'currentTime', 'duration', 'buffered',
  'videoWidth', 'videoHeight', 'playbackRate', 'volume', 'muted', 'quality', 'qualities', 'error'];
const empty = Object.freeze({ paused: true, ended: false, buffering: false, seeking: false, readyState: 'empty', currentTime: 0,
  duration: null, buffered: 0, videoWidth: 0, videoHeight: 0, playbackRate: 1, volume: 1, muted: false, quality: 'auto', qualities: Object.freeze([]), error: null });
const players = new WeakMap();
// Each mounted reader of a name, with the fields it read on its last render.
const readers = new Map();
const NAME = /^[A-Za-z0-9_.-]{1,64}$/;

/** The player name a `player` prop carries, or undefined when it is not a `useVideo` handle. */
export const playerName = handle => players.get(handle);

export function useVideo(name) {
  if (typeof name !== 'string' || !NAME.test(name)) throw Error('UI_VIDEO: useVideo takes a name of 1..64 letters, digits, "_", "." or "-"');
  const binding = local(name, empty, 'video');
  const { cell } = cellOf(binding);
  let reader = cell.videoReader;
  if (!reader) {
    reader = cell.videoReader = { binding, read: new Set() };
    if (!readers.has(name)) readers.set(name, new Set());
    readers.get(name).add(reader);
  }
  reader.read.clear();
  const handle = {};
  for (const field of fields) Object.defineProperty(handle, field, { enumerable: true, get() {
    if (render.active) reader.read.add(field);
    return binding.value[field];
  } });
  const command = (op, value) => {
    outsideRender('video command');
    const failure = globalThis.__lingxiaVideoCommand?.(`name:${name}`, op, typeof value === 'number' ? value : 0, typeof value === 'string' ? value : '')
      ?? 'VIDEO_NO_PLAYER: this host plays no video';
    if (!failure) return Promise.resolve();
    const [code] = failure.split(':', 1);
    return Promise.reject(Object.assign(Error(failure), { code, recoverable: code === 'VIDEO_NETWORK' }));
  };
  const number = (what, value, min, max) => {
    if (typeof value !== 'number' || !Number.isFinite(value) || value < min || value > max) throw Error(`UI_VIDEO: ${what} takes a number in ${min}..${max}`);
    return value;
  };
  Object.assign(handle, {
    play: () => command('play'),
    pause: () => command('pause'),
    seek: seconds => command('seek', number('seek', seconds, 0, 1e7)),
    setMuted: muted => { if (typeof muted !== 'boolean') throw Error('UI_VIDEO: setMuted takes a boolean'); return command('muted', +muted); },
    setVolume: volume => command('volume', number('setVolume', volume, 0, 1)),
    setPlaybackRate: rate => command('rate', number('setPlaybackRate', rate, 0.25, 4)),
    setQuality: quality => {
      if (typeof quality !== 'string' || !quality) throw Error('UI_VIDEO: setQuality takes "auto" or a label of sources');
      return command('quality', quality);
    },
    retry: () => command('retry'),
  });
  Object.freeze(handle);
  players.set(handle, name);
  return handle;
}

// Lists and errors arrive as new objects each tick; equal content is no change.
const same = (a, b) => Object.is(a, b) || (typeof a === 'object' && typeof b === 'object' && a !== null && b !== null && JSON.stringify(a) === JSON.stringify(b));

/** A status from the player `name`: readers re-render only for fields they read. */
export function videoStatus(name, status) {
  const next = Object.freeze({ ...status, qualities: Object.freeze([...(status.qualities ?? [])]), error: status.error ? Object.freeze({ ...status.error }) : null });
  const named = readers.get(name);
  for (const reader of named ?? []) {
    const { key, cell } = cellOf(reader.binding);
    if (page.locals.get(key) !== cell) { named.delete(reader); continue; }
    const current = reader.binding.value;
    if ([...reader.read].some(field => !same(current[field], next[field]))) reader.binding.set(next);
    else cell.value = next;
  }
}
