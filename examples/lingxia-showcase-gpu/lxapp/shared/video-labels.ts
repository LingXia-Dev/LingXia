import { t } from '@lingxia/view';
import type { VideoControlsLabels } from '@liulidesign/core/VideoControls';

export const VIDEO_LABELS: VideoControlsLabels = {
  play: 'Play', pause: 'Pause', replay: 'Replay', mute: 'Mute', unmute: 'Unmute', seek: 'Seek',
  fullscreen: 'Full screen', exitFullscreen: 'Exit full screen', settings: 'Playback settings',
  rate: 'Playback speed', quality: 'Quality', auto: 'Auto', live: 'LIVE', loading: 'Loading',
  failed: 'This video cannot play right now', retry: 'Retry',
};

/** The controls' labels in the page's language; call it while rendering. The spec fixture keeps `VIDEO_LABELS`. */
export const videoLabels = (): VideoControlsLabels =>
  Object.fromEntries(Object.keys(VIDEO_LABELS).map(name => [name, t(`video.controls.${name}`)])) as unknown as VideoControlsLabels;
