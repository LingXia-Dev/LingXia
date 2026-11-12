// `<VideoControls player>`: the standard bar for a `useVideo` player: play/pause (replay once
// ended), a scrubber with the time or a Live badge, mute, a quality menu when the player has
// qualities, full screen, and the error with Retry when it is recoverable. `overlay` lays it over
// the picture (in a positioned parent) with a loading ring. Full screen is one more view of the
// same player, in the top layer.
import { h } from './nodes.js';
import { useLocal, useView } from './hooks.js';
import { playerName, useVideo, fullscreenPlayer, posterOf } from './video.js';
import { Progress } from './progress.js';

const glyph = (d, filled = true) => filled
  ? [{ d, fill: 'currentColor' }]
  : [{ d, fill: 'none', stroke: 'currentColor', strokeWidth: 2, strokeLinecap: 'round', strokeLinejoin: 'round' }];
const speaker = 'M4 9v6h4l5 4V5L8 9z';
const icons = {
  play: glyph('M8 5.5v13l10.5-6.5z'),
  pause: glyph('M6.5 5h4v14h-4zM13.5 5h4v14h-4z'),
  replay: glyph('M4 12a8 8 0 1 0 2.3-5.6M4 4v4.5h4.5', false),
  sound: [...glyph(speaker), ...glyph('M16 9a4 4 0 0 1 0 6M18.5 6.5a7.5 7.5 0 0 1 0 11', false)],
  muted: [...glyph(speaker), ...glyph('M16 9.5l5 5M21 9.5l-5 5', false)],
  enter: glyph('M4 9V4h5M15 4h5v5M20 15v5h-5M9 20H4v-5', false),
  exit: glyph('M9 4v5H4M20 9h-5V4M15 20v-5h5M4 15h5v5', false),
};
// A dragged position holds the thumb until the player reports a time near it, or this long.
const HOLD_MS = 1500;
// Holds the player has reached: later times move the thumb again, wherever they are.
const reached = new WeakSet();

/** `m:ss`, or `h:mm:ss` from an hour. */
export const clock = seconds => {
  const s = Math.max(0, Math.floor(seconds));
  const m = Math.floor(s / 60);
  const two = n => String(n).padStart(2, '0');
  return m >= 60 ? `${Math.floor(m / 60)}:${two(m % 60)}:${two(s % 60)}` : `${m}:${two(s % 60)}`;
};

// A finger-sized target around each 22-point icon.
const iconButton = (color, testId, label, icon, onClick, extra = {}) => h('button', { 'data-testid': testId, 'aria-label': label, onClick, ...extra,
  style: { paddingTop: 11, paddingBottom: 11, paddingLeft: 11, paddingRight: 11, backgroundColor: 'transparent', color } },
  h('vector', { 'aria-hidden': true, paths: icons[icon], viewBox: [0, 0, 24, 24], style: { width: 22, height: 22 } }));

// The overlay: the bar over the picture's bottom edge on a scrim, as a native player lays it out.
const fill = { position: 'absolute', top: 0, left: 0, right: 0, bottom: 0 };
const scrim = { direction: 'bottom', stops: [{ offset: 0, color: '#00000000' }, { offset: 1, color: '#000000b3' }] };
const fade = { properties: ['opacity'], durationMs: 300, delayMs: 0, easing: [0.4, 0, 0.2, 1] };
// Seconds of playback a touch keeps the overlay up; measured on the player's clock, as the View has no timers.
const SHOWN_S = 3;
// The window edges the overlay keeps clear of (full screen).
const edges = Symbol('edges');

export function VideoControls({ player, overlay = false, className, 'data-testid': testId, [edges]: edge }) {
  if (playerName(player) === undefined) throw Error('UI_PROP: player must be a useVideo handle');
  if (typeof overlay !== 'boolean') throw Error('UI_PROP: overlay must be boolean');
  const id = part => testId && `${testId}-${part}`;
  const held = useLocal('held', null);
  // A play asked for before the player has opened: it waits on that open.
  const asked = useLocal('asked', false);
  const shown = useLocal('shown', { since: 0, hidden: false });
  const { paused, ended, muted, duration, readyState, qualities, buffering, error, fullscreen } = player;
  const live = duration === null && readyState !== 'empty';
  const known = typeof duration === 'number' && duration > 0;
  const now = player.currentTime;
  const hold = held.value;
  if (hold !== null && Math.abs(now - hold.value) <= 0.5) reached.add(hold);
  const holding = hold !== null && !reached.has(hold) && Date.now() - hold.at < HOLD_MS;
  const at = known ? Math.min(holding ? hold.value : now, duration) : 0;
  const loading = !error && (((!paused || asked.value) && readyState !== 'ready') || (!paused && buffering));
  // Any use of the overlay brings it back for another stretch.
  const wake = fn => (...args) => { if (overlay) shown.set({ since: player.currentTime, hidden: false }); return fn(...args); };
  const scrub = wake(value => {
    held.set({ value, at: Date.now() });
    player.seek(value).catch(() => held.set(null));
  });
  const toggle = wake(() => {
    asked.set(paused || ended);
    return ended ? player.seek(0).then(() => player.play()) : paused ? player.play() : player.pause();
  });
  const label = ended ? 'Replay' : paused ? 'Play' : 'Pause';
  const fg = overlay ? '#ffffff' : 'text';
  const errorRow = error && h('view', { 'data-testid': id('error'), role: 'alert', style: { flexDirection: 'row', alignItems: 'center', gap: 8, paddingLeft: 11, paddingRight: 11 } },
    h('text', { style: { flexGrow: 1, flexBasis: 0, fontSize: 12, color: 'danger' } }, error.message),
    error.recoverable && h('button', { 'data-testid': id('retry'), onClick: wake(() => player.retry()), style: { fontSize: 12, color: fg } }, 'Retry'));
  const play = iconButton(fg, id('play'), label, ended ? 'replay' : paused ? 'play' : 'pause', toggle, { 'aria-busy': loading || buffering });
  const track = live
    ? h('text', { 'data-testid': id('live'), style: { flexGrow: 1, fontSize: 12, fontWeight: 600, color: 'accent' } }, 'LIVE')
    : h('slider', { 'data-testid': id('scrub'), 'aria-label': 'Seek',
        min: 0, max: known ? duration : 1, step: 0.1, value: at, disabled: !known, onChange: scrub,
        style: { flexGrow: 1, flexBasis: 0 } });
  const time = !live && h('text', { 'data-testid': id('time'), style: { fontSize: 12, color: overlay ? '#ffffffcc' : 'text-muted', fontVariantNumeric: 'tabular-nums' } },
    `${clock(at)} / ${known ? clock(duration) : '--:--'}`);
  const mute = iconButton(fg, id('mute'), muted ? 'Unmute' : 'Mute', muted ? 'muted' : 'sound', wake(() => player.setMuted(!muted)), { 'aria-pressed': muted });
  const quality = qualities.length > 0 && h('select', { 'data-testid': id('quality'), 'aria-label': 'Quality', value: player.quality,
    options: [{ value: 'auto', label: 'Auto' }, ...qualities.map(name => ({ value: name, label: name }))],
    onChange: wake(quality => player.setQuality(quality)), style: { fontSize: 12, ...(overlay ? { color: fg, backgroundColor: '#ffffff2e', borderWidth: 0 } : {}) } });
  const screen = iconButton(fg, id('fullscreen'), fullscreen ? 'Exit full screen' : 'Full screen', fullscreen ? 'exit' : 'enter',
    wake(() => (fullscreen ? player.exitFullscreen() : player.requestFullscreen())), { 'aria-pressed': fullscreen });
  if (!overlay) {
    return h('view', { className, 'data-testid': testId, ...(className ? {} : { style: { color: fg } }) },
      errorRow,
      h('view', { style: { flexDirection: 'row', alignItems: 'center', gap: 2 } }, play, track, time, mute, quality, screen));
  }
  // A touch screen hides the bar after a stretch of playback; a pointer has no hover to bring it back yet.
  const autoHide = useView().env?.pointer === 'coarse';
  const { since, hidden } = shown.value;
  const elapsed = now >= since ? now - since : now;
  const playing = !paused && !ended;
  const up = loading || !!error || !playing || (!hidden && !(autoHide && elapsed >= SHOWN_S));
  const pad = edge ?? { left: 0, right: 0, bottom: 0 };
  // The bar stays in the tree while faded, so a screen reader still reaches every control; the
  // surface above it takes the touch that brings it back.
  return h('view', { className, 'data-testid': testId, style: { ...fill, color: fg } },
    h('button', { 'data-testid': id('surface'), 'aria-hidden': true,
      onClick: () => shown.set(up && playing ? { since: player.currentTime, hidden: true } : { since: player.currentTime, hidden: false }),
      style: { ...fill, zIndex: up ? 0 : 2, backgroundColor: 'transparent' } }),
    loading && h('view', { style: { ...fill, alignItems: 'center', justifyContent: 'center' } },
      h(Progress, { variant: 'ring', size: 36, 'aria-label': 'Loading', 'data-testid': id('loading') })),
    h('view', { 'data-testid': id('bar'), style: { position: 'absolute', left: 0, right: 0, bottom: 0, zIndex: 1, opacity: up ? 1 : 0, transition: fade,
      backgroundImage: scrim, paddingTop: 24, paddingLeft: pad.left, paddingRight: pad.right, paddingBottom: pad.bottom } },
      errorRow,
      h('view', { style: { flexDirection: 'row', alignItems: 'center', gap: 8, paddingLeft: 11, paddingRight: 11 } }, track, time),
      h('view', { style: { flexDirection: 'row', alignItems: 'center' } }, play, mute, h('view', { style: { flexGrow: 1 } }), quality, screen)));
}
VideoControls.__lxid = '@lingxia/view#VideoControls';

// The full screen view: the player again, filling the window over the page and its bars, with the
// overlay on it. A close request (back) leaves full screen.
function FullscreenVideo({ name }) {
  const player = useVideo(name);
  // The window's edges hold the notch and the home indicator: the bar stays clear of them.
  const safeArea = useView().env?.safeArea ?? { left: 0, right: 0, bottom: 0 };
  return h('dialog', { open: true, closedby: 'closerequest', onClose: () => player.exitFullscreen(), 'aria-label': 'Full screen video',
    'data-testid': 'lx-fullscreen-video',
    style: { width: '100%', height: '100%', maxWidth: '100%', maxHeight: '100%', marginTop: 0, marginBottom: 0, marginLeft: 0, marginRight: 0,
      paddingTop: 0, paddingBottom: 0, paddingLeft: 0, paddingRight: 0, borderRadius: 0, backgroundColor: '#000000' } },
    h('view', { style: { flexGrow: 1, justifyContent: 'center' } },
      // Until the player has a frame the full screen view shows what the page's view does.
      h('video', { player, poster: posterOf(name), style: { width: '100%', flexGrow: 1, objectFit: 'contain' } }),
      h(VideoControls, { player, overlay: true, [edges]: safeArea, 'data-testid': 'lx-fullscreen-controls' })));
}

/** The top-layer view of the player shown full screen, or null. */
export function fullscreenNode() {
  const name = fullscreenPlayer();
  return name === null ? null : h(FullscreenVideo, { name, key: `lx-fullscreen:${name}` });
}
