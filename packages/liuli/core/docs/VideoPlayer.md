<!-- Generated from the doc file by scripts/generate.mjs; do not edit. -->

# VideoPlayer

A video with its controls, over the picture or under it, and full screen. Status: preview.

```ts
import { VideoPlayer } from '@liulidesign/core/VideoPlayer';
```

VideoPlayer shows a `useVideo` player with VideoControls: play, seek, mute, speed and quality, and full screen as a dialog over the page. The page keeps the player, so Logic and the page drive the same video.

## Examples

### Controls over the picture

```tsx
import { useVideo } from '@lingxia/view';
import type { VideoControlsLabels } from '@liulidesign/core/VideoControls';
import { VideoPlayer } from '@liulidesign/core/VideoPlayer';

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
```

### Controls under the picture

```tsx
import { useVideo } from '@lingxia/view';
import type { VideoControlsLabels } from '@liulidesign/core/VideoControls';
import { VideoPlayer } from '@liulidesign/core/VideoPlayer';

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

/** The controls in a row under the picture. */
export function UnderThePicture() {
  const player = useVideo('liuli-player-bar');
  return <VideoPlayer player={player} label="Big Buck Bunny" labels={labels} src={sources[0].src} poster={poster} variant="bar" data-testid="under" />;
}
```

## Props

| Prop | Type | Default | Description |
| --- | --- | --- | --- |
| `player` (required) | `Video` |  | The `useVideo` player it shows; the page and Logic drive the same one. |
| `label` (required) | `string` |  | What the video shows, its accessible name. |
| `labels` (required) | `VideoControlsLabels` |  | Every word the controls show or speak. |
| `src` | `string` |  | One file or stream. Give it or `sources`. |
| `sources` | `readonly VideoSource[]` |  | The same video at several qualities; their labels become the quality menu. |
| `poster` | `string` |  | Shown until the first frame, here and full screen. |
| `variant` | `VideoPlayerVariant` | `'overlay'` | `overlay` lays the controls over the picture; `bar` puts them in a row under it. |
| `isAutoPlay` | `boolean` | `false` | Starts playing once it can, muted where the platform requires it. |
| `isLooping` | `boolean` | `false` | Starts again at the end. |
| `onError` | `((error: VideoError) => void)` |  | Runs when the video fails; the controls already show the error and Retry. |
| `className` | `string` |  | Classes for the frame, such as its aspect ratio or width. |
| `data-testid` | `string` |  | On the video; the controls take `<id>-controls`. |

## Anatomy

| Part | Element | Required | Description |
| --- | --- | --- | --- |
| video | `<video>` | yes | The picture, backed in black where it leaves room. |
| controls | `<view>` | yes | VideoControls over the picture, or in a row under it. |

## What an app may override

- `className`: Classes for the frame, such as its aspect ratio or width.
- The recipe, `videoPlayerRecipe` from `@liulidesign/core/VideoPlayer/recipe`: the classes of each part, to style an element of your own to match. The component does not read a changed copy.
- Scale tokens it reads, set in the app's stylesheet under `@theme`: `--radius-container`.
- It paints with no colour role of its own.

## Accessibility

- Pattern: none.
- Name: label.
- Description: none.
- The video is named by `label`; every control is named by `labels`.

| Contrast | Pair | Minimum | Light | Dark |
| --- | --- | --- | --- | --- |
| controls | `on-media` on `media` | 4.5:1 | 7.94:1 | 7.94:1 |

Ratios are computed from @lingxia/view/theme and rounded down.

## Guidance

- Do: Give it a poster, so the box is never blank before the first frame.
- Do: Pass `sources` when the video comes at several qualities.
- Don't: Start a video with sound on its own; use `isAutoPlay` only for muted or silent clips.

Related: VideoControls.
