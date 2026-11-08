import { List, type View } from '@lingxia/view';
import '../../theme/index.css';

const clips = ['public/video/pattern.mp4', 'public/video/fractal.mp4'];
// Every third row plays a clip under a text overlay, so a scroll moves video between plain rows.
const rows = Array.from({ length: 24 }, (_, index) => ({ id: String(index), clip: index % 3 === 1 ? clips[(index / 3) % 2 | 0] : null }));
const items = [{ id: 'hero', clip: null }, ...rows];

export function render(_: View) {
  return (
    <page>
      <List
        items={items}
        getKey={row => row.id}
        className="gap-3 p-4"
        renderItem={row =>
          row.id === 'hero' ? (
            // Unsized: a full-width 16:9 box.
            <video data-testid="video-hero" src={clips[0]} aria-label="Test pattern" className="rounded-2xl" />
          ) : row.clip ? (
            <stack data-testid={`video-row-${row.id}`} className="w-full aspect-video rounded-2xl overflow-hidden bg-surface-sunken">
              <video src={row.clip} className="w-full h-full object-cover" />
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
