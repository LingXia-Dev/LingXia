import { List, useVideo, type View } from '@lingxia/view';
import '../../theme/index.css';

const clips = ['public/video/pattern.mp4', 'public/video/fractal.mp4'];
// Every third row plays a clip under a text overlay, so a scroll moves video between plain rows.
const rows = Array.from({ length: 24 }, (_, index) => ({ id: String(index), clip: index % 3 === 1 ? clips[(index / 3) % 2 | 0] : null }));
const items = [{ id: 'hero', clip: null }, ...rows];

function Hero() {
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
    </column>
  );
}

export function render(_: View) {
  return (
    <page>
      <List
        items={items}
        getKey={row => row.id}
        className="gap-3 p-4"
        renderItem={row =>
          row.id === 'hero' ? (
            <Hero />
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
