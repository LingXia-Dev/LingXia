// Fixtures for specs only, never listed in the catalogue: Liuli's video controls over a player.
import { Progress, useLocal, useVideo, useView } from '@lingxia/view';
import { VideoControls } from '@liulidesign/core/VideoControls';
import { PUBLIC_STREAM, PUBLIC_VIDEO } from '../public-video';
import { VIDEO_LABELS } from '../video-labels';

export function SpecVideo() {
  const { data } = useView();
  const clip = useVideo('lab-clip');
  const stream = useVideo('lab-stream');
  // Specs pass `media=packaged` for the bundled clip; a network failure falls back to it too.
  const spec = (data as { packagedMedia?: boolean }).packagedMedia === true;
  // Kept once set: the bundled clip clears the player's error, which must not bring the network clip back.
  const fellBack = useLocal('lab-video-fell-back', false);
  const packaged = spec || fellBack.value;
  const source = packaged
    ? { src: 'public/video/fractal.mp4', poster: 'public/video/fractal-poster.jpg', 'aria-label': 'Fractal zoom' }
    : { sources: PUBLIC_VIDEO, 'aria-label': 'Big Buck Bunny' };
  return (
    <view data-testid="lab-video" className="gap-3">
      <view className="relative rounded-xl overflow-hidden">
        <video data-testid="lab-video-player" player={clip} {...source} loop={!packaged}
          onError={error => { if (error.code === 'VIDEO_NETWORK') fellBack.set(true); }} />
        <VideoControls player={clip} labels={VIDEO_LABELS} poster={packaged ? 'public/video/fractal-poster.jpg' : undefined} data-testid="lab-video-controls" />
      </view>
      {/* A spinner beside the player commits the page every frame; the bar must still follow it. */}
      {spec ? <Progress variant="ring" size={24} aria-label="Loading next" data-testid="lab-video-spinner" /> : (
        <view className="relative rounded-xl overflow-hidden">
          <video data-testid="lab-stream-player" player={stream} src={PUBLIC_STREAM} aria-label="HLS test stream" />
          <VideoControls player={stream} labels={VIDEO_LABELS} data-testid="lab-stream-controls" />
        </view>
      )}
    </view>
  );
}
