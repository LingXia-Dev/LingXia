import { useVideo } from '@lingxia/view';
import { VideoControls, type VideoControlsLabels } from './VideoControls';

// Big Buck Bunny (c) Blender Foundation, CC BY 3.0: a 10 s public clip.
const clip = 'https://test-videos.co.uk/vids/bigbuckbunny/mp4/h264/720/Big_Buck_Bunny_720_10s_1MB.mp4';
const poster = 'https://peach.blender.org/wp-content/uploads/title_anouncement.jpg';

const labels: VideoControlsLabels = {
  play: 'Play', pause: 'Pause', replay: 'Replay', mute: 'Mute', unmute: 'Unmute', seek: 'Seek',
  fullscreen: 'Full screen', exitFullscreen: 'Exit full screen', settings: 'Playback settings',
  rate: 'Playback speed', quality: 'Quality', auto: 'Auto', live: 'LIVE', loading: 'Loading',
  failed: 'This video cannot play right now', retry: 'Retry',
};

/** Over the picture: a large play first, then a quiet bar that hides on touch while it plays. */
export function Overlay() {
  const player = useVideo('liuli-controls-overlay');
  return (
    <view className="relative aspect-video overflow-hidden rounded-xl">
      <video player={player} src={clip} poster={poster} aria-label="Big Buck Bunny" className="size-full" />
      <VideoControls player={player} labels={labels} poster={poster} data-testid="overlay-controls" />
    </view>
  );
}

/** A row under the picture, for a small player or a page that keeps the picture clear. */
export function Bar() {
  const player = useVideo('liuli-controls-bar');
  return (
    <view className="gap-1">
      <view className="aspect-video overflow-hidden rounded-xl">
        <video player={player} src={clip} poster={poster} aria-label="Big Buck Bunny" className="size-full" />
      </view>
      <VideoControls player={player} labels={labels} variant="bar" poster={poster} data-testid="bar-controls" />
    </view>
  );
}

/** The home tile. */
export function Specimen() {
  const player = useVideo('liuli-controls-specimen');
  return (
    <view className="relative aspect-video w-28 overflow-hidden rounded-lg">
      <VideoControls player={player} labels={labels} />
    </view>
  );
}
