import { List, useVideo } from '@lingxia/view';
import { VideoControls } from '@liulidesign/core/VideoControls';
import '../../theme/index.css';
import { PUBLIC_STREAM, PUBLIC_VIDEO } from '../../shared/public-video';
import { VIDEO_LABELS } from '../../shared/video-labels';
import type { View } from './$types';

const clips = ['public/video/pattern.mp4', 'public/video/fractal.mp4'];
// Every third row plays a clip under a text overlay, so a scroll moves video between plain rows.
const rows = Array.from({ length: 24 }, (_, index) => ({ id: String(index), clip: index % 3 === 1 ? clips[(index / 3) % 2 | 0] : null }));
const items = Object.freeze([{ id: 'hero', clip: null }, { id: 'portrait', clip: null }, { id: 'quality', clip: null }, { id: 'public', clip: null }, { id: 'stream', clip: null }, ...rows]);
const sources = [{ src: 'public/video/q360.mp4', label: '360p' }, { src: 'public/video/q720.mp4', label: '720p' }];

// A tall clip: full screen keeps a phone upright instead of turning it.
function Portrait() {
  const video = useVideo('portrait');
  return (
    <view className="gap-2">
      <view className="items-center">
        <video data-testid="video-portrait" player={video} src="public/video/portrait.mp4" muted loop aria-label="Portrait clip" className="w-36 rounded-xl" />
      </view>
      {/* Under the clip at full width: beside it the bar ran out of room for full screen. */}
      <VideoControls player={video} labels={VIDEO_LABELS} variant="bar" data-testid="video-portrait-controls" />
    </view>
  );
}

function Hero({ actions }: { actions: View['actions'] }) {
  const hero = useVideo('hero');
  return (
    <view className="gap-2">
      {/* Unsized: a full-width 16:9 box. */}
      <video data-testid="video-hero" player={hero} src={clips[0]} autoPlay muted loop aria-label="Test pattern" className="rounded-2xl" />
      <view className="flex-row items-center justify-between gap-3">
        <text data-testid="video-hero-state" className="text-muted">
          {hero.paused ? 'paused' : 'playing'} · {hero.currentTime.toFixed(1)}s / {(hero.duration ?? 0).toFixed(1)}s
        </text>
        <button
          data-testid="video-hero-toggle"
          aria-label={hero.paused ? 'Play' : 'Pause'}
          onClick={() => (hero.paused ? hero.play() : hero.pause())}
          className="px-4 py-2 rounded-xl bg-surface text-text active:bg-surface-sunken"
        >
          <text>{hero.paused ? 'Play' : 'Pause'}</text>
        </button>
      </view>
      <view className="flex-row gap-2">
        <button data-testid="video-hero-logic-pause" onClick={() => actions.pauseHero()} className="px-3 py-2 rounded-xl bg-surface text-text active:bg-surface-sunken">
          <text>Pause from Logic</text>
        </button>
        <button data-testid="video-hero-logic-play" onClick={() => actions.playHero()} className="px-3 py-2 rounded-xl bg-surface text-text active:bg-surface-sunken">
          <text>Play from Logic</text>
        </button>
      </view>
    </view>
  );
}

// Two files of one clip: a switch keeps the position and never shows black.
function Quality({ actions }: { actions: View['actions'] }) {
  const video = useVideo('quality');
  return (
    <view className="gap-2">
      <video data-testid="video-quality" player={video} sources={sources} autoPlay muted loop aria-label="Quality test" className="rounded-2xl" />
      <view className="flex-row items-center justify-between gap-2">
        <text data-testid="video-quality-state" className="text-muted">
          {video.quality} · {video.videoHeight}p · {video.currentTime.toFixed(1)}s
        </text>
        <view className="flex-row gap-2">
          {['auto', ...video.qualities].map(quality => (
            <button
              key={quality}
              data-testid={`video-quality-${quality}`}
              aria-pressed={video.quality === quality}
              onClick={() => video.setQuality(quality)}
              className={video.quality === quality ? 'px-3 py-2 rounded-xl bg-accent-solid text-on-accent-solid' : 'px-3 py-2 rounded-xl bg-surface text-text'}
            >
              <text>{quality}</text>
            </button>
          ))}
        </view>
      </view>
      <button data-testid="video-quality-logic-360p" onClick={() => actions.qualityFromLogic({ quality: '360p' })} className="px-3 py-2 rounded-xl bg-surface text-text active:bg-surface-sunken">
        <text>360p from Logic</text>
      </button>
    </view>
  );
}

function PublicQuality() {
  const video = useVideo('public');
  return (
    <view className="gap-2">
      <video data-testid="video-public" player={video} sources={PUBLIC_VIDEO} autoPlay muted loop aria-label="Big Buck Bunny" className="rounded-2xl" />
      <text data-testid="video-public-state" className="text-muted">
        {video.quality} · {video.videoHeight}p · {video.qualities.join('/')}{video.error ? ` · ${video.error.code}` : ''}
      </text>
      <view className="flex-row flex-wrap gap-2">
        {['auto', ...video.qualities].map(quality => (
          <button
            key={quality}
            data-testid={`video-public-${quality}`}
            aria-pressed={video.quality === quality}
            onClick={() => video.setQuality(quality)}
            className={video.quality === quality ? 'px-3 py-2 rounded-xl bg-accent-solid text-on-accent-solid' : 'px-3 py-2 rounded-xl bg-surface text-text'}
          >
            <text>{quality}</text>
          </button>
        ))}
      </view>
    </view>
  );
}

function Stream() {
  const video = useVideo('stream');
  return (
    <view className="gap-2">
      <video data-testid="video-stream" player={video} src={PUBLIC_STREAM} autoPlay muted loop aria-label="HLS test" className="rounded-2xl" />
      <text data-testid="video-stream-state" className="text-muted">
        {video.quality} · {video.videoHeight}p · {video.qualities.join('/')} · {video.currentTime.toFixed(1)}s{video.error ? ` · ${video.error.code}` : ''}
      </text>
      <view className="flex-row flex-wrap gap-2">
        {['auto', ...video.qualities].map(quality => (
          <button key={quality} data-testid={`video-stream-${quality}`} onClick={() => video.setQuality(quality)} className="px-3 py-2 rounded-xl bg-surface text-text">
            <text>{quality}</text>
          </button>
        ))}
      </view>
    </view>
  );
}

export function render({ actions }: View) {
  return (
    <page>
      <List
        items={items}
        getKey={row => row.id}
        className="gap-3 p-4"
        renderItem={row =>
          row.id === 'hero' ? (
            <Hero actions={actions} />
          ) : row.id === 'portrait' ? (
            <Portrait />
          ) : row.id === 'quality' ? (
            <Quality actions={actions} />
          ) : row.id === 'public' ? (
            <PublicQuality />
          ) : row.id === 'stream' ? (
            <Stream />
          ) : row.clip ? (
            <view data-testid={`video-row-${row.id}`} className="grid *:col-start-1 *:row-start-1 w-full aspect-video rounded-2xl overflow-hidden bg-surface-sunken">
              <video src={row.clip} autoPlay muted loop className="w-full h-full object-cover" />
              <text className="self-end p-3 text-text font-semibold">Row {row.id} · overlay above video</text>
            </view>
          ) : (
            <view className="flex-row h-16 items-center px-4 rounded-xl bg-surface"><text>Row {row.id}</text></view>
          )
        }
      />
    </page>
  );
}
