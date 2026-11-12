// `<VideoControls player>`: the standard controls for a `useVideo` player: play/pause (replay once
// ended), seek with the time or a Live badge, mute, playback speed and a quality menu when the
// player has qualities, full screen, and the error with Retry when it is recoverable. `overlay`
// lays them over the picture (in a positioned parent that holds only the video). Full screen is
// one more view of the same player, in the top layer.
import { h } from './nodes.js';
import { useLocal, useView } from './hooks.js';
import { playerName, useVideo, fullscreenPlayer, posterOf } from './video.js';

// One set on a 24 grid with one round line: play and pause filled, the rest drawn, sized so each
// carries the weight of the settings sliders.
const W = 1.75;
const solid = d => ({ d, fill: 'currentColor', stroke: 'currentColor', strokeWidth: W, strokeLinejoin: 'round', strokeLinecap: 'round' });
const line = d => ({ d, fill: 'none', stroke: 'currentColor', strokeWidth: W, strokeLinejoin: 'round', strokeLinecap: 'round' });
const speaker = 'M5 10v4h2.75l4 3.25V6.75l-4 3.25z';
const icons = {
  play: [solid('M8.25 6.6v10.8a.75.75 0 0 0 1.14.64l8.6-5.4a.75.75 0 0 0 0-1.28l-8.6-5.4a.75.75 0 0 0-1.14.64z')],
  pause: [solid('M8.25 6.5h1.25v11H8.25zM14.5 6.5h1.25v11H14.5z')],
  replay: [line('M5.5 12a6.5 6.5 0 1 0 1.9-4.6M5.5 4.9v3.35h3.35')],
  sound: [line(speaker), line('M15 9.75a3.25 3.25 0 0 1 0 4.5M17.5 7.5a6.5 6.5 0 0 1 0 9')],
  muted: [line(speaker), line('M15.5 10l4 4M19.5 10l-4 4')],
  enter: [line('M5.5 9.25v-2.5a1.25 1.25 0 0 1 1.25-1.25h2.5M14.75 5.5h2.5a1.25 1.25 0 0 1 1.25 1.25v2.5M18.5 14.75v2.5a1.25 1.25 0 0 1-1.25 1.25h-2.5M9.25 18.5h-2.5a1.25 1.25 0 0 1-1.25-1.25v-2.5')],
  exit: [line('M9.25 5.5V8A1.25 1.25 0 0 1 8 9.25H5.5M18.5 9.25H16A1.25 1.25 0 0 1 14.75 8V5.5M14.75 18.5V16A1.25 1.25 0 0 1 16 14.75h2.5M5.5 14.75H8A1.25 1.25 0 0 1 9.25 16v2.5')],
  settings: [line('M4.75 8h7.5M17.25 8h2M4.75 16h2M11.75 16h7.5'), line('M12.25 8a2.5 2.5 0 1 0 5 0a2.5 2.5 0 1 0-5 0M6.75 16a2.5 2.5 0 1 0 5 0a2.5 2.5 0 1 0-5 0')],
  close: [line('M6.5 9.5l5.5 5.5 5.5-5.5')],
};
const glyph = (name, size = 20) => h('vector', { 'aria-hidden': true, paths: icons[name], viewBox: [0, 0, 24, 24], style: { width: size, height: size } });

// The words the controls say; `labels` puts them in the page's language.
const LABELS = Object.freeze({ play: 'Play', pause: 'Pause', replay: 'Replay', mute: 'Mute', unmute: 'Unmute', seek: 'Seek',
  fullscreen: 'Full screen', exitFullscreen: 'Exit full screen', quality: 'Quality', auto: 'Auto', rate: 'Playback speed',
  live: 'LIVE', retry: 'Retry', loading: 'Loading', settings: 'Playback settings', failed: 'This video cannot play right now' });
const RATES = [0.5, 0.75, 1, 1.25, 1.5, 2];
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

// A 44-point target around each icon until hit slop lands.
const iconButton = (color, testId, label, icon, onClick, extra = {}, size = 20, box = 44) => h('button', { 'data-testid': testId, 'aria-label': label, onClick, ...extra,
  style: { width: box, height: box, alignItems: 'center', justifyContent: 'center', backgroundColor: 'transparent', color } },
  glyph(icon, size));

// Over the picture the media roles hold in both appearances; the theme's accent marks progress.
const INK = 'on-media';
const fill = { position: 'absolute', top: 0, left: 0, right: 0, bottom: 0 };
const soft = [0.25, 0.1, 0.25, 1];
const fadeMotion = { properties: ['opacity', 'transform'], durationMs: 220, delayMs: 0, easing: soft };
const spin = { name: 'lx-video-spin', durationMs: 900, delayMs: 0, iterations: null, whenReduced: 'base', easing: 'linear', opacity: null,
  transforms: [{ kind: 'rotate', angle: [{ offset: 0, value: 0 }, { offset: 1, value: 360 }] }] };
// Glass without a backdrop, which would be recaptured every frame over a moving picture.
const glass = { backgroundColor: 'media' };

// Seconds of playback a touch keeps the overlay up; measured on the player's clock, as the View has no timers.
const SHOWN_S = 3;
// The window edges the overlay keeps clear of (full screen).
const edges = Symbol('edges');
// The page's labels by player, so its full screen view speaks the same language.
const spoken = new Map();

export function VideoControls({ player, overlay = false, labels, className, 'data-testid': testId, [edges]: edge }) {
  if (playerName(player) === undefined) throw Error('UI_PROP: player must be a useVideo handle');
  if (typeof overlay !== 'boolean') throw Error('UI_PROP: overlay must be boolean');
  const say = labels ? { ...LABELS, ...labels } : edge ? spoken.get(playerName(player)) ?? LABELS : LABELS;
  if (!edge) spoken.set(playerName(player), say);
  const id = part => testId && `${testId}-${part}`;
  const held = useLocal('held', null);
  // A play asked for before the player has opened: it waits on that open.
  const asked = useLocal('asked', false);
  // A view mounted mid-play (full screen) starts with its controls up for a stretch.
  const shown = useLocal('shown', { since: player.currentTime, hidden: false });
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
  // The stretch counts from where the seek lands, or the jump itself would hide the bar.
  const scrub = value => {
    if (overlay) shown.set({ since: value, hidden: false });
    held.set({ value, at: Date.now() });
    player.seek(value).catch(() => held.set(null));
  };
  const toggle = wake(() => {
    asked.set(paused || ended);
    return ended ? player.seek(0).then(() => player.play()) : paused ? player.play() : player.pause();
  });
  const retry = wake(() => { asked.set(true); return player.retry().then(() => player.play()); });
  const playLabel = ended ? say.replay : paused ? say.play : say.pause;
  const playIcon = ended ? 'replay' : paused ? 'play' : 'pause';
  const seekProps = { 'data-testid': id('scrub'), 'aria-label': say.seek, min: 0, max: known ? duration : 1, step: 0.1, value: at, disabled: !known, onChange: scrub };
  const qualityOptions = [{ value: 'auto', label: say.auto }, ...qualities.map(name => ({ value: name, label: name }))];
  const full = edge !== undefined;
  // Full screen gives every control a larger target.
  const box = full ? 48 : 44;
  const size = full ? 22 : 20;
  const fg = overlay ? INK : 'text';
  const mute = iconButton(fg, id('mute'), muted ? say.unmute : say.mute, muted ? 'muted' : 'sound', wake(() => player.setMuted(!muted)), { 'aria-pressed': muted }, size, box);
  const screen = iconButton(fg, id('fullscreen'), fullscreen ? say.exitFullscreen : say.fullscreen, fullscreen ? 'exit' : 'enter',
    wake(() => (fullscreen ? player.exitFullscreen() : player.requestFullscreen())), { 'aria-pressed': fullscreen }, size, box);
  const play = iconButton(fg, id('play'), playLabel, playIcon, toggle, { 'aria-busy': loading || buffering }, size, box);
  if (!overlay) {
    return h('view', { className, 'data-testid': testId, ...(className ? {} : { style: { color: fg } }) },
      error && h('view', { 'data-testid': id('error'), role: 'alert', style: { flexDirection: 'row', alignItems: 'center', gap: 8, paddingLeft: 11, paddingRight: 11 } },
        h('text', { style: { flexGrow: 1, flexBasis: 0, fontSize: 12, color: 'danger' } }, error.message),
        error.recoverable && h('button', { 'data-testid': id('retry'), onClick: () => player.retry(), style: { fontSize: 12 } }, say.retry)),
      h('view', { style: { flexDirection: 'row', alignItems: 'center', gap: 2 } },
        play,
        live
          ? h('text', { 'data-testid': id('live'), style: { flexGrow: 1, fontSize: 12, fontWeight: 600, color: 'accent' } }, say.live)
          : h('slider', { ...seekProps, style: { flexGrow: 1, flexBasis: 0 } }),
        !live && h('text', { 'data-testid': id('time'), style: { fontSize: 12, color: 'text-muted', fontVariantNumeric: 'tabular-nums' } },
          `${clock(at)} / ${known ? clock(duration) : '--:--'}`),
        mute,
        qualities.length > 0 && h('select', { 'data-testid': id('quality'), 'aria-label': say.quality, value: player.quality, options: qualityOptions,
          onChange: quality => player.setQuality(quality), style: { fontSize: 12 } }),
        screen));
  }

  const env = useView().env;
  const precise = env?.pointer === 'fine';
  const menu = useLocal('menu', false);
  const { since, hidden } = shown.value;
  const elapsed = now >= since ? now - since : now;
  const playing = !paused && !ended;
  const fresh = paused && !ended && !asked.value && now === 0 && !error;
  // Kept up whatever the pointer does: paused, loading, an error, an open menu, a drag in progress.
  const pinned = loading || !!error || !playing || holding || menu.value;
  // Touch: a tap shows the controls; a stretch of playback or a tap on the picture hides them.
  // A precise pointer keeps them up: a component cannot follow hover or focus yet. Before the
  // first play only the large play shows.
  const up = !fresh && (pinned || precise || (!hidden && elapsed < SHOWN_S));
  const pad = edge ?? { top: 0, left: 0, right: 0, bottom: 0 };

  const time = live
    ? h('text', { 'data-testid': id('live'), style: { fontSize: 12, fontWeight: 600, color: 'accent', paddingLeft: 4 } }, say.live)
    : h('text', { 'data-testid': id('time'), style: { fontSize: full ? 13 : 12, color: INK, fontVariantNumeric: 'tabular-nums', paddingLeft: 2 } },
      clock(at), h('span', { style: { color: 'on-media/70' } }, ` / ${known ? clock(duration) : '--:--'}`));

  // The scrubber is the bar's only line: one slider whose track ends sit half its thumb inside the
  // box. Its look follows the bar's own drag, as a runtime component has no part states. A media
  // timeline runs left to right in every language.
  const scrubber = style => !live && h('slider', { ...seekProps, dir: 'ltr',
    secondaryValue: known ? Math.min(duration, now + (player.buffered ?? 0)) : 0,
    'aria-valuetext': `${clock(at)} / ${known ? clock(duration) : '--:--'}`,
    style: { position: 'absolute', height: 28, paddingLeft: 0, paddingRight: 0, ...style,
      trackPart: { height: holding ? 5 : 3, borderRadius: 999, backgroundColor: 'on-media/25' },
      secondaryPart: { backgroundColor: 'on-media/45' },
      rangePart: { backgroundColor: 'accent' },
      // Always 12 wide, so the track ends stay put; it shows while held.
      thumbPart: { width: 12, height: 12, borderRadius: 999, backgroundColor: holding ? 'on-media' : 'transparent' } } });

  const settings = h('button', { 'data-testid': id('settings'), 'aria-label': say.settings, 'aria-expanded': menu.value, onClick: wake(() => menu.set(!menu.value)),
    style: { width: box, height: box, alignItems: 'center', justifyContent: 'center', backgroundColor: 'transparent', color: INK } }, glyph('settings', size));
  const left = h('view', { style: { flexDirection: 'row', alignItems: 'center', flexShrink: 0 } }, play, time);
  const right = h('view', { style: { flexDirection: 'row', alignItems: 'center', flexShrink: 0 } }, settings, mute, screen);
  // The line sits in the bar's top band, clear of its edge and of the icons below it.
  const lineAt = full ? 12 : 10;
  const inset = full ? 16 : 8;
  const radius = full ? 20 : 16;
  const barHeight = lineAt + 4 + box + (full ? 4 : 0);
  const lift = { transition: fadeMotion };
  const bar = h('view', { 'data-testid': id('bar'), style: { position: 'absolute', left: inset + pad.left, right: inset + pad.right, bottom: (full ? 12 : 8) + pad.bottom,
    ...glass, borderRadius: radius, paddingTop: lineAt + 4, paddingBottom: full ? 4 : 0, paddingLeft: 2, paddingRight: 2, ...lift } },
    scrubber({ top: lineAt - 14, left: 12, right: 12 }),
    h('view', { style: { flexDirection: 'row', alignItems: 'center', justifyContent: 'space-between' } }, left, right));
  // Full screen: a way out at the top start, clear of the notch.
  const close = full && h('view', { style: { position: 'absolute', top: 12 + (pad.top ?? 0), left: inset + pad.left, ...lift } },
    h('view', { style: { ...glass, borderRadius: 999 } },
      iconButton(INK, id('close'), say.exitFullscreen, 'close', wake(() => player.exitFullscreen()), {}, size, box)));

  // Speed and quality as labelled rows of chips in the box rather than a popover, docked where the bar is: it opens upward from the bar when
  // the box has room, and slides down over the bar when it does not, so it never leaves the video.
  const pick = fn => wake(() => { menu.set(false); return fn(); });
  const chip = (testId, label, chosen, onClick) => h('button', { 'data-testid': testId, 'aria-pressed': chosen, onClick: pick(onClick),
    style: { height: 44, minWidth: 48, paddingLeft: 10, paddingRight: 10, borderRadius: 12, alignItems: 'center', justifyContent: 'center',
      backgroundColor: chosen ? 'accent' : 'on-media/25' } },
    h('text', { style: { fontSize: 14, fontWeight: chosen ? 600 : 400, color: chosen ? 'on-accent' : INK, fontVariantNumeric: 'tabular-nums' } }, label));
  const chips = (label, items) => h('view', { role: 'group', 'aria-label': label, style: { flexDirection: 'row', flexWrap: 'wrap', gap: 6 } }, ...items);
  const sheet = menu.value && h('view', { style: { ...fill, zIndex: 3 } },
    // A tap anywhere else on the picture closes it.
    h('button', { 'data-testid': id('menu-dismiss'), 'aria-hidden': true, onClick: () => menu.set(false), style: { ...fill, backgroundColor: 'transparent' } }),
    h('view', { style: { position: 'absolute', top: inset + pad.top, bottom: (full ? 12 : 8) + pad.bottom, left: inset + pad.left, right: inset + pad.right,
      alignItems: 'flex-end', justifyContent: 'flex-end' } },
      h('view', { 'data-testid': id('menu'), role: 'group', 'aria-label': say.settings,
        style: { flexShrink: 1, overflow: 'hidden', ...glass, borderRadius: radius } },
        // Two rows without headings fit above the bar of a phone's 16:9 inline video; the rows' names are for screen readers.
        h('view', { style: { gap: 8, paddingTop: 8, paddingBottom: 8, paddingLeft: 8, paddingRight: 8 } },
          chips(say.rate, RATES.map(r => chip(id(`rate-${r}`), `${r}×`, player.playbackRate === r, () => player.setPlaybackRate(r)))),
          qualities.length > 0 && chips(say.quality, ['auto', ...qualities].map(q => chip(id(`quality-${q}`), q === 'auto' ? say.auto : q, player.quality === q, () => player.setQuality(q)))))),
      // Holds the menu above the bar while there is room, and gives way first when there is not.
      h('view', { style: { height: barHeight + 8, flexShrink: 1000 } })));

  const disc = full ? 72 : 60;
  const errorPanel = error && h('view', { 'data-testid': id('error'), role: 'alert', 'aria-label': say.failed,
    style: { ...glass, borderRadius: 18, alignItems: 'center', gap: 10, maxWidth: 280, paddingTop: 14, paddingBottom: error.recoverable ? 12 : 14, paddingLeft: 18, paddingRight: 18 } },
    h('text', { style: { fontSize: 14, fontWeight: 500, color: INK, textAlign: 'center' } }, say.failed),
    error.recoverable && h('button', { 'data-testid': id('retry'), onClick: retry,
      style: { height: 44, minWidth: 96, paddingLeft: 18, paddingRight: 18, borderRadius: 999, alignItems: 'center', justifyContent: 'center', backgroundColor: 'on-media/25' } },
      h('text', { style: { fontSize: 14, fontWeight: 600, color: INK } }, say.retry)));
  const centre = errorPanel || (loading
    ? h('view', { role: 'progressbar', 'aria-label': say.loading, 'data-testid': id('loading'),
        style: { ...glass, width: disc, height: disc, borderRadius: 999, alignItems: 'center', justifyContent: 'center' } },
      h('vector', { 'aria-hidden': true, viewBox: [0, 0, 24, 24], style: { width: disc * 0.46, height: disc * 0.46, animation: spin },
        paths: [{ d: 'M12 3.5a8.5 8.5 0 1 1 0 17a8.5 8.5 0 1 1 0-17', fill: 'none', stroke: 'on-media/25', strokeWidth: W },
          { d: 'M12 3.5a8.5 8.5 0 0 1 8.5 8.5', fill: 'none', stroke: INK, strokeWidth: W, strokeLinecap: 'round' }] }))
    // The bar's play stays the one a screen reader hears.
    : fresh && h('button', { 'data-testid': id('start'), 'aria-label': say.play, 'aria-hidden': true, onClick: toggle,
        style: { ...glass, width: disc, height: disc, borderRadius: 999, alignItems: 'center', justifyContent: 'center', color: INK, paddingLeft: 2 } }, glyph('play', disc * 0.5)));

  // The faded chrome stays in the tree, so a screen reader still reaches every control. On touch
  // the surface takes the tap that brings it back; on a precise pointer a click plays or pauses.
  return h('view', { className, 'data-testid': testId, style: { ...fill, color: INK } },
    h('button', { 'data-testid': id('surface'), 'aria-hidden': true,
      onClick: precise || fresh ? toggle : () => shown.set({ since: player.currentTime, hidden: up && playing }),
      style: { ...fill, zIndex: up || fresh ? 0 : 2, backgroundColor: 'transparent' } }),
    centre && h('view', { style: { ...fill, alignItems: 'center', justifyContent: 'center', zIndex: 1 } }, centre),
    h('view', { 'data-testid': id('chrome'), style: { ...fill, zIndex: 1, opacity: up ? 1 : 0, transition: fadeMotion } }, bar, close),
    sheet);
}
VideoControls.__lxid = '@lingxia/view#VideoControls';

// The full screen view: the player again, filling the window over the page and its bars, with the
// overlay on it. A close request (back) leaves full screen.
function FullscreenVideo({ name }) {
  const player = useVideo(name);
  // The window's edges hold the notch and the home indicator: the controls stay clear of them.
  const safeArea = { top: 0, left: 0, right: 0, bottom: 0, ...useView().env?.safeArea };
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
