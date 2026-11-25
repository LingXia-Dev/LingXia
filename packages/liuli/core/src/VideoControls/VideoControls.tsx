import { cx, Fragment, useLocal, useView, type Video, type VectorPath } from '@lingxia/view';
import { liuliText } from '../i18n/text';
import { dataOf } from '../internal/data';
import { glyphGrowth, useTextGrowth } from '../internal/Glyph';
import { useLargeText } from '../internal/largeText';
import { videoControlsRecipe as r } from './VideoControls.recipe';

export type VideoControlsVariant = 'overlay' | 'bar';

/** The words the controls show and speak, in the page's language. */
export interface VideoControlsLabels {
  play: string; pause: string; replay: string; mute: string; unmute: string; seek: string;
  fullscreen: string; exitFullscreen: string; settings: string; rate: string; quality: string; auto: string;
  live: string; loading: string; failed: string; retry: string;
}

export interface VideoControlsProps {
  /** The `useVideo` player they drive. */
  player: Video;
  /** Words to use instead of Liuli's, which follow the page's language. */
  labels?: Partial<VideoControlsLabels>;
  /**
   * `overlay` lays them over the picture: put them after the `<video>` in a `relative` parent that
   * holds only that video. `bar` is a row under it. @default 'overlay'
   */
  variant?: VideoControlsVariant;
  /** The picture full screen shows until the player has a frame; pass the `<video>`'s poster. */
  poster?: string;
  className?: string;
  /** Prefixes each control's test id: `<id>-play`, `<id>-scrub`, `<id>-fullscreen`. */
  'data-testid'?: string;
}

const names: (keyof VideoControlsLabels)[] = [
  'play', 'pause', 'replay', 'mute', 'unmute', 'seek', 'fullscreen', 'exitFullscreen', 'settings', 'rate', 'quality', 'auto', 'live',
  'loading', 'failed', 'retry',
];
const videoLabels = (given: Partial<VideoControlsLabels> = {}): VideoControlsLabels =>
  Object.fromEntries(names.map(name => [name, given[name] ?? liuliText(`@liuli.video.${name}`)])) as unknown as VideoControlsLabels;

// One set on a 24 grid with one round line: play and pause filled, the rest drawn.
const W = 1.75;
const solid = (d: string): VectorPath => ({ d, fill: 'currentColor', stroke: 'currentColor', strokeWidth: W, strokeLinejoin: 'round', strokeLinecap: 'round' });
const line = (d: string): VectorPath => ({ d, fill: 'none', stroke: 'currentColor', strokeWidth: W, strokeLinejoin: 'round', strokeLinecap: 'round' });
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
  spinner: [
    { d: 'M12 3.5a8.5 8.5 0 1 1 0 17a8.5 8.5 0 1 1 0-17', fill: 'none', stroke: 'on-media/25', strokeWidth: W },
    { d: 'M12 3.5a8.5 8.5 0 0 1 8.5 8.5', fill: 'none', stroke: 'currentColor', strokeWidth: W, strokeLinecap: 'round' },
  ],
} as const satisfies Record<string, readonly VectorPath[]>;
type Icon = keyof typeof icons;
const box = [0, 0, 24, 24] as const;

const RATES = [0.5, 0.75, 1, 1.25, 1.5, 2];
// Seconds of playback a touch keeps the overlay up; measured on the player's clock, as there are no timers.
const SHOWN_S = 3;
// Drags whose seek has begun, and drags the player has reached: a held position shows until then.
const seen = new WeakSet<object>();
const reached = new WeakSet<object>();
// Plays the player has answered: it played, ended or failed. A paused clock that moves is a seek
// (a resume lands before the first frame), not an answer; a second tap takes the play back.
const answered = new WeakSet<object>();

const edgeStyle = (area?: { top: number; right: number; bottom: number; left: number }) =>
  ({ top: area?.top ?? 0, right: area?.right ?? 0, bottom: area?.bottom ?? 0, left: area?.left ?? 0 });

/** `m:ss`, or `h:mm:ss` from an hour; the digits read the same in every language. */
function clock(seconds: number) {
  const s = Math.max(0, Math.floor(seconds));
  const m = Math.floor(s / 60);
  const two = (n: number) => String(n).padStart(2, '0');
  return m >= 60 ? `${Math.floor(m / 60)}:${two(m % 60)}:${two(s % 60)}` : `${m}:${two(s % 60)}`;
}

/**
 * The standard controls for a `useVideo` player: play and pause (replay once ended), a scrubber
 * that holds a dragged position until the player reaches it, the time or a Live badge, mute, speed
 * and quality, full screen, and a recoverable error's Retry. Full screen is a dialog over the page
 * that shows the same player again with these controls.
 */
export function VideoControls({ player, labels: given, variant = 'overlay', poster, className, 'data-testid': testId, ...attributes }: VideoControlsProps) {
  const labels = videoLabels(given);
  // The dialog follows the inline controls, so entering full screen keeps their state.
  return (
    <Fragment>
      <Controls player={player} labels={labels} variant={variant} className={className} testId={testId} data={dataOf(attributes)} />
      {player.fullscreen && (
        <dialog
          open
          closedby="closerequest"
          onClose={() => { void player.exitFullscreen(); }}
          aria-label={labels.fullscreen}
          data-testid={testId && `${testId}-fullscreen-view`}
          className={r.fullscreen.panel}
        >
          <view className={r.fullscreen.stage}>
            <video player={player} poster={poster} className={r.fullscreen.video} />
            <Controls player={player} labels={labels} variant="overlay" isFull testId={testId && `${testId}-full`} />
          </view>
        </dialog>
      )}
    </Fragment>
  );
}

interface ControlsProps {
  player: Video;
  labels: VideoControlsLabels;
  variant: VideoControlsVariant;
  isFull?: boolean;
  className?: string;
  testId?: string;
  data?: Record<string, unknown>;
}

// As the recipe's `glyph` and `button` sizes, in points.
const glyphSides = { inline: 20, full: 22 };
const buttonSides = { inline: 44, full: 48 };

function Controls({ player, labels: say, variant, isFull = false, className, testId, data }: ControlsProps) {
  const id = (part: string) => testId && `${testId}-${part}`;
  const held = useLocal<{ value: number } | null>('held', null);
  // A finger on the scrubber, from the slider itself; a host without the signal leaves it false.
  const down = useLocal('down', false);
  // The last play asked, and the error showing then; the ring shows until the player answers.
  const asked = useLocal<{ error: string | null } | null>('asked', null);
  const shown = useLocal('shown', { since: player.currentTime, hidden: false });
  const menu = useLocal('menu', false);
  const env = useView().env;
  // Each icon grows with the text, as an icon button's does; under the picture its target grows by as much,
  // over it the 44 pt target already holds the grown glyph and the row must stay inside the picture.
  const glyph = useTextGrowth(glyphSides[isFull ? 'full' : 'inline'], glyphGrowth);
  const target = glyph && variant !== 'overlay' ? buttonSides[isFull ? 'full' : 'inline'] + glyph.width - glyphSides[isFull ? 'full' : 'inline'] : undefined;
  const isLarge = useLargeText();
  const { paused, ended, muted, duration, readyState, qualities, buffering, seeking, error, fullscreen } = player;
  const live = duration === null && readyState !== 'empty';
  const known = typeof duration === 'number' && duration > 0;
  const now = player.currentTime;
  const hold = held.value;
  if (hold !== null && seeking) seen.add(hold);
  if (hold !== null && (Math.abs(now - hold.value) <= 0.5 || (seen.has(hold) && !seeking))) reached.add(hold);
  const holding = down.value || (hold !== null && !reached.has(hold));
  const at = known ? Math.min(holding && hold ? hold.value : now, duration) : 0;
  const ask = asked.value;
  if (ask && (!paused || ended || (error && error.message !== ask.error))) answered.add(ask);
  const waiting = !!ask && !answered.has(ask);
  // Asked to play: the controls read as playing from the tap.
  const still = paused && !waiting;
  const loading = !error && (waiting || (!paused && readyState !== 'ready') || (!paused && buffering));
  const overlay = variant === 'overlay';
  // Any use of the overlay brings it back for another stretch.
  const wake = <A extends unknown[]>(fn: (...args: A) => unknown) => (...args: A) => {
    if (overlay) shown.set({ since: player.currentTime, hidden: false });
    return fn(...args);
  };
  // The stretch counts from where the seek lands, or the jump itself would hide the bar.
  const scrub = (value: number) => {
    if (overlay) shown.set({ since: value, hidden: false });
    held.set({ value });
    player.seek(value).catch(() => held.set(null));
  };
  const toggle = wake(() => {
    // A second tap while the play is unanswered takes it back.
    const play = (paused || ended) && !waiting;
    asked.set(play ? { error: error?.message ?? null } : null);
    return ended ? player.seek(0).then(() => player.play()) : play ? player.play() : player.pause();
  });
  const retry = wake(() => { asked.set({ error: error?.message ?? null }); return player.retry().then(() => player.play()); });
  const size = isFull ? 'full' : 'inline';
  const tone = overlay ? 'overlay' : 'bar';
  const iconButton = (part: string, label: string, icon: Icon, onClick: () => unknown, extra: Record<string, unknown> = {}) => (
    <button data-testid={id(part)} aria-label={label} onClick={onClick} {...extra} style={target ? { width: target, height: target } : undefined} className={cx(r.button.base, r.button[size], r.button[tone])}>
      <vector aria-hidden paths={icons[icon]} viewBox={box} style={glyph} className={r.glyph[size]} />
    </button>
  );
  const play = iconButton('play', ended ? say.replay : still ? say.play : say.pause, ended ? 'replay' : still ? 'play' : 'pause', toggle, { 'aria-busy': loading || buffering });
  const mute = iconButton('mute', muted ? say.unmute : say.mute, muted ? 'muted' : 'sound', wake(() => player.setMuted(!muted)), { 'aria-pressed': muted });
  const screen = iconButton('fullscreen', fullscreen ? say.exitFullscreen : say.fullscreen, fullscreen ? 'exit' : 'enter',
    wake(() => (fullscreen ? player.exitFullscreen() : player.requestFullscreen())), { 'aria-pressed': fullscreen });
  const settings = iconButton('settings', say.settings, 'settings', wake(() => menu.set(!menu.value)), { 'aria-expanded': menu.value });
  const pick = (fn: () => unknown) => wake(() => { menu.set(false); return fn(); });
  const chipTone = overlay ? r.chip.overlay : r.chip.page;
  const chip = (part: string, label: string, chosen: boolean, onClick: () => unknown) => (
    <button data-testid={id(part)} aria-pressed={chosen} onClick={pick(onClick)} className={cx(r.chip.base, chosen ? chipTone.on : chipTone.off)}>
      <text className={cx(r.chip.text, isFull ? r.chip.textFull : r.chip.textInline, chosen ? chipTone.textOn : chipTone.textOff)}>{label}</text>
    </button>
  );
  const groups = (
    <>
      <view role="group" aria-label={say.rate} className={r.menu.group}>
        {RATES.map(rate => chip(`rate-${rate}`, `${rate}×`, player.playbackRate === rate, () => player.setPlaybackRate(rate)))}
      </view>
      {qualities.length > 0 && (
        <view role="group" aria-label={say.quality} className={r.menu.group}>
          {['auto', ...qualities].map(q => chip(`quality-${q}`, q === 'auto' ? say.auto : q, player.quality === q, () => player.setQuality(q)))}
        </view>
      )}
    </>
  );
  const valueText = `${clock(at)} / ${known ? clock(duration) : '--:--'}`;
  const seek = {
    'data-testid': id('scrub'), 'aria-label': say.seek, 'aria-valuetext': valueText,
    min: 0, max: known ? duration : 1, step: 0.1, value: at, disabled: !known, onChange: scrub,
    // A new press starts from where the player is, not from an earlier drag's value.
    onPressChange: wake((pressed: boolean) => { if (pressed) held.set(null); down.set(pressed); }),
    secondaryValue: known ? Math.min(duration, now + (player.buffered ?? 0)) : 0,
    // A media timeline runs left to right in every language.
    dir: 'ltr' as const,
  };

  if (!overlay) {
    return (
      <view className={cx(r.root.bar, className)} data-testid={testId} {...data}>
        {error && (
          <view data-testid={id('error')} role="alert" className={r.barRow.error}>
            <text className={r.barRow.errorText}>{error.message}</text>
            {error.recoverable && <button data-testid={id('retry')} onClick={() => player.retry()}><text>{say.retry}</text></button>}
          </view>
        )}
        {menu.value && <view data-testid={id('menu')} role="group" aria-label={say.settings} className={r.barRow.menu}>{groups}</view>}
        {isLarge ? (
          // At large text the scrubber takes a row of its own, and the controls go under it.
          <>
            {!live && <view className={r.barRow.scrub}><slider {...seek} className={r.barRow.slider} /></view>}
            <view className={r.barRow.base}>
              {play}
              {live ? <text data-testid={id('live')} className={cx(r.live.base, r.live.bar)}>{say.live}</text> : <text data-testid={id('time')} className={cx(r.time.bar, r.time.shrinks)}>{valueText}</text>}
              <view className={r.barRow.spacer} />
              {settings}
              {mute}
              {screen}
            </view>
          </>
        ) : (
          <view className={r.barRow.base}>
            {play}
            {live ? <text data-testid={id('live')} className={cx(r.live.base, r.live.bar)}>{say.live}</text> : <slider {...seek} className={r.barRow.slider} />}
            {!live && <text data-testid={id('time')} className={r.time.bar}>{valueText}</text>}
            {settings}
            {mute}
            {screen}
          </view>
        )}
      </view>
    );
  }

  const precise = env?.pointer === 'fine';
  const { since, hidden } = shown.value;
  const elapsed = now >= since ? now - since : now;
  const playing = !paused && !ended;
  // A seek can report 0 on its way; a drag or a seek is never the first play.
  const fresh = paused && !ended && !asked.value && now === 0 && !error && !holding && !seeking;
  // Kept up whatever the pointer does: paused, loading, an error, an open menu, a drag in progress.
  const pinned = loading || !!error || !playing || holding || menu.value;
  // Touch: a tap shows the controls; a stretch of playback or a tap on the picture hides them. A
  // precise pointer keeps them up until hover reaches components. Before the first play only the
  // large play shows.
  const up = !fresh && (pinned || precise || (!hidden && elapsed < SHOWN_S));

  const time = live
    ? <text data-testid={id('live')} className={cx(r.live.base, r.live.overlay)}>{say.live}</text>
    : (
      <text data-testid={id('time')} className={cx(r.time.base, r.time[size])}>
        {clock(at)}<span className={r.time.total}>{` / ${known ? clock(duration) : '--:--'}`}</span>
      </text>
    );
  const disc = isFull ? r.disc.full : r.disc.inline;
  const centre = error
    ? (
      <view data-testid={id('error')} role="alert" aria-label={say.failed} className={r.error.base}>
        <text className={r.error.text}>{say.failed}</text>
        {error.recoverable && (
          <button data-testid={id('retry')} onClick={retry} className={r.error.retry}><text className={r.error.retryText}>{say.retry}</text></button>
        )}
      </view>
    )
    : loading
      ? (
        <view role="progressbar" aria-label={say.loading} data-testid={id('loading')} className={cx(r.disc.base, disc)}>
          <vector aria-hidden paths={icons.spinner} viewBox={box} className={r.spinner[size]} />
        </view>
      )
      // The bar's play stays the one a screen reader hears.
      : fresh && (
        <button data-testid={id('start')} aria-label={say.play} aria-hidden onClick={toggle} className={cx(r.disc.base, disc, r.start.base)}>
          <vector aria-hidden paths={icons.play} viewBox={box} className={isFull ? r.glyph.discFull : r.glyph.disc} />
        </button>
      );

  return (
    // Full screen keeps every control clear of the notch and the home indicator.
    <view className={cx(isFull ? r.edges.base : r.root.overlay, className)} style={isFull ? edgeStyle(env?.safeArea) : undefined} data-testid={testId} {...data}>
      <button
        data-testid={id('surface')}
        aria-hidden
        aria-label={still ? say.play : say.pause}
        onClick={precise || fresh ? toggle : () => shown.set({ since: player.currentTime, hidden: up && playing })}
        className={cx(r.surface.base, up || fresh ? r.surface.under : r.surface.over)}
      />
      {/* Play and Retry sit above the bar so the scrubber's touch box never takes them; the ring stays under it. */}
      {centre && <view className={cx(r.centre.base, loading && !error ? r.centre.under : r.centre.over)}>{centre}</view>}
      <view data-testid={id('chrome')} className={cx(r.chrome.base, up && !menu.value ? r.chrome.shown : r.chrome.hidden)}>
        <view data-testid={id('bar')} className={r.bar[size]}>
          {!live && <slider {...seek} className={cx(r.scrubber.base, r.scrubber[size])} />}
          <view className={r.row.base}>
            <view className={r.row.group}>{play}{time}</view>
            <view className={r.row.group}>{settings}{mute}{screen}</view>
          </view>
        </view>
        {isFull && <view className={r.close.full}>{iconButton('close', say.exitFullscreen, 'close', wake(() => player.exitFullscreen()))}</view>}
      </view>
      {menu.value && (
        <view className={r.menuLayer.base}>
          <button data-testid={id('menu-dismiss')} aria-hidden aria-label={say.settings} onClick={() => menu.set(false)} className={r.menuLayer.dismiss} />
          <view className={r.menuLayer[size]}>
            <view data-testid={id('menu')} role="group" aria-label={say.settings} className={cx(r.menu.base, r.menu[size])}>
              <view className={r.menu.body}>{groups}</view>
            </view>
          </view>
        </view>
      )}
    </view>
  );
}
