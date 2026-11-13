import { useVideo } from '@lingxia/view';
import type { VideoControlsLabels } from '../VideoControls/VideoControls';
import { VideoPlayer } from './VideoPlayer';

// Big Buck Bunny (c) Blender Foundation, CC BY 3.0: one 10 s clip at three public qualities.
const sources = [360, 720, 1080].map(height => ({
  src: `https://test-videos.co.uk/vids/bigbuckbunny/mp4/h264/${height}/Big_Buck_Bunny_${height}_10s_1MB.mp4`,
  label: `${height}p`,
}));
const poster = 'https://peach.blender.org/wp-content/uploads/title_anouncement.jpg';

const labels: VideoControlsLabels = {
  play: 'Play', pause: 'Pause', replay: 'Replay', mute: 'Mute', unmute: 'Unmute', seek: 'Seek',
  fullscreen: 'Full screen', exitFullscreen: 'Exit full screen', settings: 'Playback settings',
  rate: 'Playback speed', quality: 'Quality', auto: 'Auto', live: 'LIVE', loading: 'Loading',
  failed: 'This video cannot play right now', retry: 'Retry',
};

/** The picture with its controls over it; the qualities fill the settings menu. */
export function Standard() {
  const player = useVideo('liuli-player-standard');
  return <VideoPlayer player={player} label="Big Buck Bunny" labels={labels} sources={sources} poster={poster} data-testid="standard" />;
}

/** The controls in a row under the picture. */
export function UnderThePicture() {
  const player = useVideo('liuli-player-bar');
  return <VideoPlayer player={player} label="Big Buck Bunny" labels={labels} src={sources[0].src} poster={poster} variant="bar" data-testid="under" />;
}

/** The home tile. */
export function Specimen() {
  const player = useVideo('liuli-player-specimen');
  return <VideoPlayer player={player} label="Big Buck Bunny" labels={labels} src={sources[0].src} poster={poster} className="w-28" />;
}
