// `<VideoControls player>`: the standard bar for a `useVideo` player: play/pause (replay once
// ended), a scrubber with the time or a Live badge, mute, a quality menu when the player has
// qualities, and the error with Retry when it is recoverable.
import { h } from './nodes.js';
import { useLocal } from './hooks.js';
import { playerName } from './video.js';

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
const iconButton = (testId, label, icon, onClick, extra = {}) => h('button', { 'data-testid': testId, 'aria-label': label, onClick, ...extra,
  style: { paddingTop: 11, paddingBottom: 11, paddingLeft: 11, paddingRight: 11, backgroundColor: 'transparent', color: 'text' } },
  h('vector', { 'aria-hidden': true, paths: icons[icon], viewBox: [0, 0, 24, 24], style: { width: 22, height: 22 } }));

export function VideoControls({ player, className, 'data-testid': testId }) {
  if (playerName(player) === undefined) throw Error('UI_PROP: player must be a useVideo handle');
  const id = part => testId && `${testId}-${part}`;
  const held = useLocal('held', null);
  const { paused, ended, muted, duration, readyState, qualities, buffering, error } = player;
  const live = duration === null && readyState !== 'empty';
  const known = typeof duration === 'number' && duration > 0;
  const now = player.currentTime;
  const hold = held.value;
  if (hold !== null && Math.abs(now - hold.value) <= 0.5) reached.add(hold);
  const holding = hold !== null && !reached.has(hold) && Date.now() - hold.at < HOLD_MS;
  const at = known ? Math.min(holding ? hold.value : now, duration) : 0;
  const scrub = value => {
    held.set({ value, at: Date.now() });
    player.seek(value).catch(() => held.set(null));
  };
  const toggle = () => ended ? player.seek(0).then(() => player.play()) : paused ? player.play() : player.pause();
  const label = ended ? 'Replay' : paused ? 'Play' : 'Pause';
  return h('column', { className, 'data-testid': testId, style: { color: 'text' } },
    error && h('row', { 'data-testid': id('error'), role: 'alert', style: { alignItems: 'center', gap: 8, paddingLeft: 11, paddingRight: 11 } },
      h('text', { style: { flexGrow: 1, flexBasis: 0, fontSize: 12, color: 'danger' } }, error.message),
      error.recoverable && h('button', { 'data-testid': id('retry'), onClick: () => player.retry(), style: { fontSize: 12 } }, 'Retry')),
    h('row', { style: { alignItems: 'center', gap: 2 } },
      iconButton(id('play'), label, ended ? 'replay' : paused ? 'play' : 'pause', toggle, { 'aria-busy': buffering }),
      live
        ? h('text', { 'data-testid': id('live'), style: { flexGrow: 1, fontSize: 12, fontWeight: 600, color: 'accent' } }, 'LIVE')
        : h('slider', { 'data-testid': id('scrub'), 'aria-label': 'Seek',
            min: 0, max: known ? duration : 1, step: 0.1, value: at, disabled: !known, onChange: scrub,
            style: { flexGrow: 1, flexBasis: 0 } }),
      !live && h('text', { 'data-testid': id('time'), style: { fontSize: 12, color: 'text-muted', fontVariantNumeric: 'tabular-nums' } },
        `${clock(at)} / ${known ? clock(duration) : '--:--'}`),
      iconButton(id('mute'), muted ? 'Unmute' : 'Mute', muted ? 'muted' : 'sound', () => player.setMuted(!muted), { 'aria-pressed': muted }),
      qualities.length > 0 && h('select', { 'data-testid': id('quality'), 'aria-label': 'Quality', value: player.quality,
        options: [{ value: 'auto', label: 'Auto' }, ...qualities.map(name => ({ value: name, label: name }))],
        onChange: quality => player.setQuality(quality), style: { fontSize: 12 } })));
}
VideoControls.__lxid = '@lingxia/view#VideoControls';
