<!-- Generated from the doc file by scripts/generate.mjs; do not edit. -->

# Slider

A value picked along a track, with its label and the value in words. Status: preview.

```ts
import { Slider } from '@liulidesign/core/Slider';
```

Slider picks one number from a range: a volume, a size, a position in a recording. A label row names it and can show the value in words the app writes; the track and thumb are the primitive slider.

## Examples

### With the value in words

```tsx
import { useLocal } from '@lingxia/view';
import { Slider } from '@liulidesign/core/Slider';

/** The value in words at the end of the label row; the app writes it. */
export function Volume() {
  const volume = useLocal('volume', 40);
  return <Slider bind={volume} label="Volume" valueLabel={`${volume.value}%`} data-testid="volume" />;
}
```

### Whole steps, with help text

```tsx
import { useLocal } from '@lingxia/view';
import { Slider } from '@liulidesign/core/Slider';

/** Whole steps: the value snaps to each one. */
export function Stepped() {
  const size = useLocal('size', 3);
  return (
    <Slider
      bind={size}
      label="Text size"
      valueLabel={['Smallest', 'Small', 'Default', 'Large', 'Largest'][size.value - 1]}
      min={1}
      max={5}
      description="Applies to notes and the reader."
      data-testid="text-size"
    />
  );
}
```

### Sizes

```tsx
import { useLocal } from '@lingxia/view';
import { Slider } from '@liulidesign/core/Slider';

export function Sizes() {
  const level = useLocal('level', 60);
  return (
    <view className="gap-4">
      <Slider bind={level} label="Small" size="sm" valueLabel={`${level.value}%`} />
      <Slider bind={level} label="Medium" valueLabel={`${level.value}%`} />
    </view>
  );
}
```

### A position in a recording

```tsx
import { useLocal } from '@lingxia/view';
import { Slider } from '@liulidesign/core/Slider';

/** A position in a recording: the label is hidden, the times stay visible. */
export function Position() {
  const at = useLocal('at', 84);
  const time = (seconds: number) => `${Math.floor(seconds / 60)}:${String(seconds % 60).padStart(2, '0')}`;
  return (
    <view className="gap-1">
      <text className="text-body font-medium text-text">Interview with the architect</text>
      <Slider bind={at} label="Position" isLabelHidden valueLabel={`${time(at.value)} of 3:10`} max={190} data-testid="position" />
    </view>
  );
}
```

## Props

| Prop | Type | Default | Description |
| --- | --- | --- | --- |
| `bind` | `Local<number>` |  | A View-local cell the slider reads and writes; use it or `value` with `onChange`. |
| `value` | `number` |  | The app's value; pass `onChange` with it. |
| `onChange` | `((value: number) => void)` |  | Runs with each new value while dragging, snapped to `step`. |
| `label` (required) | `string` |  | The visible label and the slider's accessible name. |
| `isLabelHidden` | `boolean` | `false` | Hides the visible label; the slider keeps it as its name. |
| `valueLabel` | `string` |  | The value in words, drawn at the end of the label row and read after the name: "40%", "1:24 of 3:10". Liuli formats no numbers. |
| `description` | `string` |  | Help text under the slider, read as its description. |
| `min` | `number` | `0` |  |
| `max` | `number` | `100` |  |
| `step` | `number` | `1` | The value snaps to it. |
| `size` | `SliderSize` | `'md'` |  |
| `isDisabled` | `boolean` | `false` |  |
| `className` | `string` |  |  |
| `data-testid` | `string` |  | On the slider. |

## Anatomy

| Part | Element | Required | Description |
| --- | --- | --- | --- |
| label | `<text>` | yes | The visible label; visually hidden with `isLabelHidden`, still the name. |
| value | `<text>` | no | The value in words, from `valueLabel`; read as the description. |
| control | `<slider>` | yes | The slider primitive: track, filled part and thumb, and the accessible node. |
| description | `<text>` | no | Help text, read after the value. |

## Accessibility

- Pattern: slider.
- Name: label.
- Description: valueLabel, then description.
- States: disabled (from isDisabled).
- Left / Down, Right / Up: One step down or up..
- Home, End: The minimum or the maximum..
- Press target: sm 32 px, md 40 px.
- The device reads the number as the value; the words in `valueLabel` follow as the description until sliders take a value text.
- The thumb is 24 px at both sizes; the size changes the press area and the label row.

| Contrast | Pair | Minimum | Proven by the theme pack |
| --- | --- | --- | --- |
| thumb and filled track | `accent` on `surface` | 3:1 | yes |
| empty track | `line` on `surface` | 3:1 | no |

## Guidance

- Do: Show the value in words when the number alone means little: "Large", "1:24 of 3:10".
- Do: Pick a step the person can hit with a finger: at most a hundred stops along the track.
- Don't: Use a slider for an exact number the person knows; use a text input.
- Don't: Act on every change when the work is slow; apply when the value settles.

## Not yet

| Missing | Needs | Meanwhile |
| --- | --- | --- |
| A second, quieter range on the track for what is loaded (buffered media). | a secondary range value on the slider primitive, painted between track and fill | Not drawn; a media scrubber shows no buffer. |
| Liuli's own thumb: a surface disc with a hairline and shadow that grows slightly while pressed. | slider thumb and track styling (fill, border, shadow, size, pressed state) on the primitive | The primitive's solid 24 px accent thumb and 4 px track, tinted with text-accent. |
| A spoken value in words ("Large") in place of the number. | aria-valuetext on the slider primitive, as progressbar has | The words are read as the description, after the number. |
| The track lined up with the label above it. | the slider inset to read per-side padding (`p-0` sets the sides, not the shorthand) | The track starts 12 px in from the label; `p-0` falls back to the default 12 px inset. |

Related: ProgressBar.
