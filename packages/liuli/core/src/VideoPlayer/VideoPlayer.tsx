import { cx, type Video, type VideoSource, type VideoError } from '@lingxia/view';
import { VideoControls, type VideoControlsLabels } from '../VideoControls/VideoControls';
import { videoPlayerRecipe as r } from './VideoPlayer.recipe';

export type VideoPlayerVariant = 'overlay' | 'bar';

export interface VideoPlayerProps {
  /** The `useVideo` player it shows; the page and Logic drive the same one. */
  player: Video;
  /** What the video shows, its accessible name. */
  label: string;
  /** Words for the controls to use instead of Liuli's, which follow the page's language. */
  labels?: Partial<VideoControlsLabels>;
  /** One file or stream. Give it or `sources`. */
  src?: string;
  /** The same video at several qualities; their labels become the quality menu. */
  sources?: readonly VideoSource[];
  /** Shown until the first frame, here and full screen. */
  poster?: string;
  /** `overlay` lays the controls over the picture; `bar` puts them in a row under it. @default 'overlay' */
  variant?: VideoPlayerVariant;
  /** Starts playing once it can, muted where the platform requires it. @default false */
  isAutoPlay?: boolean;
  /** Starts again at the end. @default false */
  isLooping?: boolean;
  /** Runs when the video fails; the controls already show the error and Retry. */
  onError?: (error: VideoError) => void;
  /** Classes for the frame, such as its aspect ratio or width. */
  className?: string;
  /** On the video; the controls take `<id>-controls`. */
  'data-testid'?: string;
}

/**
 * A video with its controls: the picture, then the bar over it or under it, and full screen as a
 * dialog over the page. The page keeps the player, so Logic can drive it too.
 */
export function VideoPlayer({
  player, label, labels, src, sources, poster, variant = 'overlay', isAutoPlay = false, isLooping = false, onError, className,
  'data-testid': testId,
}: VideoPlayerProps) {
  const video = (
    <video
      player={player}
      src={src}
      sources={sources}
      poster={poster}
      autoPlay={isAutoPlay}
      loop={isLooping}
      aria-label={label}
      onError={onError}
      data-testid={testId}
      className={r.video.base}
    />
  );
  const controls = <VideoControls player={player} labels={labels} variant={variant} poster={poster} data-testid={testId && `${testId}-controls`} />;
  return variant === 'overlay'
    ? <view className={cx(r.root.variant.overlay, className)}>{video}{controls}</view>
    : <view className={r.root.variant.bar}><view className={cx(r.frame.base, className)}>{video}</view>{controls}</view>;
}
