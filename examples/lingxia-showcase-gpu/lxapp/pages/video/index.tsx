import { List, useVideo, type View } from '@lingxia/view';
import '../../theme/index.css';
import { PUBLIC_STREAM, PUBLIC_VIDEO } from '../../shared/public-video';

const clips = ['public/video/pattern.mp4', 'public/video/fractal.mp4'];
// Every third row plays a clip under a text overlay, so a scroll moves video between plain rows.
const rows = Array.from({ length: 24 }, (_, index) => ({ id: String(index), clip: index % 3 === 1 ? clips[(index / 3) % 2 | 0] : null }));
const items = [{ id: 'hero', clip: null }, { id: 'quality', clip: null }, { id: 'public', clip: null }, { id: 'stream', clip: null }, ...rows];
const sources = [{ src: 'public/video/q360.mp4', label: '360p' }, { src: 'public/video/q720.mp4', label: '720p' }];

function Hero({ actions }: { actions: View['actions'] }) {
  const hero = useVideo('hero');
  return (
    <column className="gap-2">
      {/* Unsized: a full-width 16:9 box. */}
      <video data-testid="video-hero" player={hero} src={clips[0]} autoPlay muted loop aria-label="Test pattern" className="rounded-2xl" />
      <row className="items-center justify-between gap-3">
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
      </row>
      <row className="gap-2">
        <button data-testid="video-hero-logic-pause" onClick={() => actions.pauseHero()} className="px-3 py-2 rounded-xl bg-surface text-text active:bg-surface-sunken">
          <text>Pause from Logic</text>
        </button>
        <button data-testid="video-hero-logic-play" onClick={() => actions.playHero()} className="px-3 py-2 rounded-xl bg-surface text-text active:bg-surface-sunken">
          <text>Play from Logic</text>
        </button>
      </row>
    </column>
  );
}

// Two files of one clip: a switch keeps the position and never shows black.
function Quality({ actions }: { actions: View['actions'] }) {
  const video = useVideo('quality');
  return (
    <column className="gap-2">
      <video data-testid="video-quality" player={video} sources={sources} autoPlay muted loop aria-label="Quality test" className="rounded-2xl" />
      <row className="items-center justify-between gap-2">
        <text data-testid="video-quality-state" className="text-muted">
          {video.quality} · {video.videoHeight}p · {video.currentTime.toFixed(1)}s
        </text>
        <row className="gap-2">
          {['auto', ...video.qualities].map(quality => (
            <button
              key={quality}
              data-testid={`video-quality-${quality}`}
              aria-pressed={video.quality === quality}
              onClick={() => video.setQuality(quality)}
              className={video.quality === quality ? 'px-3 py-2 rounded-xl bg-accent text-on-accent' : 'px-3 py-2 rounded-xl bg-surface text-text'}
            >
              <text>{quality}</text>
            </button>
          ))}
        </row>
      </row>
      <button data-testid="video-quality-logic-360p" onClick={() => actions.qualityFromLogic({ quality: '360p' })} className="px-3 py-2 rounded-xl bg-surface text-text active:bg-surface-sunken">
        <text>360p from Logic</text>
      </button>
    </column>
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
            className={video.quality === quality ? 'px-3 py-2 rounded-xl bg-accent text-on-accent' : 'px-3 py-2 rounded-xl bg-surface text-text'}
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
    <column className="gap-2">
      <video data-testid="video-stream" player={video} src={PUBLIC_STREAM} autoPlay muted loop aria-label="HLS test" className="rounded-2xl" />
      <text data-testid="video-stream-state" className="text-muted">
        {video.quality} · {video.videoHeight}p · {video.qualities.join('/')} · {video.currentTime.toFixed(1)}s{video.error ? ` · ${video.error.code}` : ''}
      </text>
      <row className="flex-wrap gap-2">
        {['auto', ...video.qualities].map(quality => (
          <button key={quality} data-testid={`video-stream-${quality}`} onClick={() => video.setQuality(quality)} className="px-3 py-2 rounded-xl bg-surface text-text">
            <text>{quality}</text>
          </button>
        ))}
      </row>
    </column>
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
          ) : row.id === 'quality' ? (
            <Quality actions={actions} />
          ) : row.id === 'public' ? (
            <PublicQuality />
          ) : row.id === 'stream' ? (
            <Stream />
          ) : row.clip ? (
            <stack data-testid={`video-row-${row.id}`} className="w-full aspect-video rounded-2xl overflow-hidden bg-surface-sunken">
              <video src={row.clip} autoPlay muted loop className="w-full h-full object-cover" />
              <text className="self-end p-3 text-text font-semibold">Row {row.id} · overlay above video</text>
            </stack>
          ) : (
            <row className="h-16 items-center px-4 rounded-xl bg-surface"><text>Row {row.id}</text></row>
          )
        }
      />
    </page>
  );
}
