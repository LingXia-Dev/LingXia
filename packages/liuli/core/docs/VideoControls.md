<!-- Generated from the doc file by scripts/generate.mjs; do not edit. -->

# VideoControls

Play, seek, mute, speed, quality and full screen for a useVideo player. Status: preview.

```ts
import { VideoControls } from '@liulidesign/core/VideoControls';
```

VideoControls drives a `useVideo` player: over the picture as a quiet bar that hides on touch while it plays, or as a row under it. Full screen is a dialog over the page that shows the same player again with these controls. Prefer `VideoPlayer`, which pairs the picture and the controls.

## Examples

### Over the picture

```tsx
import { useVideo } from '@lingxia/view';
import { VideoControls, type VideoControlsLabels } from '@liulidesign/core/VideoControls';

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
```

### As a row under the picture

```tsx
import { useVideo } from '@lingxia/view';
import { VideoControls, type VideoControlsLabels } from '@liulidesign/core/VideoControls';

const clip = 'https://test-videos.co.uk/vids/bigbuckbunny/mp4/h264/720/Big_Buck_Bunny_720_10s_1MB.mp4';
const poster = 'https://peach.blender.org/wp-content/uploads/title_anouncement.jpg';
const labels: VideoControlsLabels = {
  play: 'Play', pause: 'Pause', replay: 'Replay', mute: 'Mute', unmute: 'Unmute', seek: 'Seek',
  fullscreen: 'Full screen', exitFullscreen: 'Exit full screen', settings: 'Playback settings',
  rate: 'Playback speed', quality: 'Quality', auto: 'Auto', live: 'LIVE', loading: 'Loading',
  failed: 'This video cannot play right now', retry: 'Retry',
};

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
```

## Props

| Prop | Type | Default | Description |
| --- | --- | --- | --- |
| `player` (required) | `Video` |  | The `useVideo` player they drive. |
| `labels` (required) | `VideoControlsLabels` |  | Every word they show or speak. |
| `variant` | `VideoControlsVariant` | `'overlay'` | `overlay` lays them over the picture: put them after the `<video>` in a `relative` parent that holds only that video. `bar` is a row under it. |
| `poster` | `string` |  | The picture full screen shows until the player has a frame; pass the `<video>`'s poster. |
| `className` | `string` |  |  |
| `data-testid` | `string` |  | Prefixes each control's test id: `<id>-play`, `<id>-scrub`, `<id>-fullscreen`. |

## VideoControlsLabels props

| Prop | Type | Default | Description |
| --- | --- | --- | --- |
| `play` (required) | `string` |  |  |
| `pause` (required) | `string` |  |  |
| `replay` (required) | `string` |  |  |
| `mute` (required) | `string` |  |  |
| `unmute` (required) | `string` |  |  |
| `seek` (required) | `string` |  |  |
| `fullscreen` (required) | `string` |  |  |
| `exitFullscreen` (required) | `string` |  |  |
| `settings` (required) | `string` |  |  |
| `rate` (required) | `string` |  |  |
| `quality` (required) | `string` |  |  |
| `auto` (required) | `string` |  |  |
| `live` (required) | `string` |  |  |
| `loading` (required) | `string` |  |  |
| `failed` (required) | `string` |  |  |
| `retry` (required) | `string` |  |  |

## Anatomy

| Part | Element | Required | Description |
| --- | --- | --- | --- |
| surface | `<button>` | yes | The picture under the bar: a tap shows or hides the bar on touch, plays or pauses with a mouse. |
| bar | `<view>` | yes | Media glass at the bottom edge: the scrubber line, then play and the time, then settings, mute and full screen. Under the picture, one row: play, the slider, the time, settings, mute and full screen. |
| scrubber | `<slider>` | no | A slider whose touch box grows up into the picture; it thickens and shows its thumb while pressed. Absent for live streams. |
| centre | `<view>` | no | A large play before the first play, a ring from a play tap until the player answers and while loading, or the error with Retry. |
| menu | `<view>` | no | Speed and quality, a row of chips each, in the bar's place while open. |
| fullscreen | `<dialog>` | no | A dialog showing the player again, full window, with the same controls and a way out. |

## What an app may override

- `className`: classes merged after the recipe's, so they win.
- The recipe, `videoControlsRecipe` from `@liulidesign/core/VideoControls/recipe`: the classes of each part, to style an element of your own to match. The component does not read a changed copy.
- Scale tokens it reads, set in the app's stylesheet under `@theme`: `--spacing-control-md`, `--text-caption`, `--text-footnote`, `--text-body`, `--shadow-contact`, `--radius-container`, `--radius-dialog`.
- Colour roles it paints with, set by the theme pack: `accent`, `danger`, `focus`, `knob`, `line`, `line-strong`, `media`, `on-accent`, `on-media`, `text`, `text-muted`.

## Accessibility

- Pattern: none.
- Name: none; its content is read.
- Description: none.
- States: pressed (from mute and full screen); expanded (from the settings menu); busy (from play while the player loads).
- Arrow keys on the scrubber: seek by 0.1 s steps.
- Escape: leaves full screen.
- The scrubber reads its position as "m:ss / m:ss" and runs left to right in every language.
- Over the picture the accent is not used: progress, the chosen chip and LIVE paint in the media roles.
- Faded controls stay in the tree, so a screen reader reaches every one of them.

| Contrast | Pair | Minimum | Light | Dark |
| --- | --- | --- | --- | --- |
| icons and time | `on-media` on `media` | 4.5:1 | 7.94:1 | 7.94:1 |
| progress line | `on-media` on `media` | 3:1 | 7.94:1 | 7.94:1 |
| progress against the unplayed track | `on-media` on `on-media/25` over `media` | 3:1 | 4.14:1 | 4.14:1 |
| chosen speed or quality | `media` on `on-media` | 4.5:1 | 7.94:1 | 7.94:1 |
| speed or quality under the picture | `text` on `text/8` over `surface` | 4.5:1 | 13.91:1 | 11.29:1 |
| chosen speed or quality under the picture | `on-accent` on `accent` | 4.5:1 | 5.62:1 | 7.12:1 |

Ratios are computed from @lingxia/view/theme and rounded down.

## Guidance

- Do: Put overlay controls after the `<video>` in a `relative` parent that holds only that video.
- Do: Pass the `<video>`'s poster so full screen shows it before the first frame.
- Do: Pass every label in the page's language.
- Don't: Lay anything else over the picture between the video and its controls.
- Don't: Put a backdrop blur behind the bar; the picture moves.

## Not yet

| Missing | Needs | Meanwhile |
| --- | --- | --- |
| Hiding the bar when a mouse leaves the picture. | pointer enter and leave on components | with a mouse the bar stays up. |
| Double-tap to skip. | a double-tap gesture | drag the scrubber. |

Related: VideoPlayer, Slider.
