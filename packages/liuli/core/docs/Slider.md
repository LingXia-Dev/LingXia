<!-- Generated from the doc file by scripts/generate.mjs; do not edit. -->

# Slider

A value picked along a track, with its label and the value in words. Status: preview.

```ts
import { Slider } from '@liulidesign/core/Slider';
```

Slider picks one number from a range: a volume, a size, a position in a recording. A label row names it and can show the value in words the app writes, which is also what a screen reader says. A second, quieter range can show how much of a recording has loaded.

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

### As a list row

```tsx
import { useLocal } from '@lingxia/view';
import { Item } from '@liulidesign/core/Item';
import { Slider } from '@liulidesign/core/Slider';

/** Among list rows: the row's label style, the value at the end. The stage's own padding is undone so the rows meet its edges. */
export function InList() {
  const speed = useLocal('speed', 2);
  return (
    <view className="-m-5 divide-y divide-text/8">
      <Item label="Read aloud" description="Notes are read in the system voice" />
      <Slider bind={speed} label="Speed" valueLabel={['Slow', 'Normal', 'Fast'][speed.value - 1]} min={1} max={3} variant="row" data-testid="speed" />
    </view>
  );
}
```

### With marks

```tsx
import { useLocal } from '@lingxia/view';
import { Slider } from '@liulidesign/core/Slider';

/** A short scale with its points named: ticks under the track, labels at the ends and the middle. */
export function Marks() {
  const size = useLocal('size', 2);
  const names = ['Small', 'Default', 'Large', 'Larger', 'Largest'];
  return (
    <Slider
      bind={size}
      label="Text size"
      valueLabel={names[size.value]}
      min={0}
      max={4}
      marks={[{ value: 0, label: 'Small' }, { value: 1 }, { value: 2 }, { value: 3 }, { value: 4, label: 'Largest' }]}
      data-testid="marked"
    />
  );
}
```

### Acting when the drag ends

```tsx
import { useLocal } from '@lingxia/view';
import { Slider } from '@liulidesign/core/Slider';

/** Saving waits for the finger to lift: the label follows the drag, the saved value follows its end. */
export function SaveOnRelease() {
  const level = useLocal('level', 60);
  const saved = useLocal('saved', 60);
  return (
    <view className="gap-2">
      <Slider bind={level} label="Brightness" valueLabel={`${level.value}%`} onChangeEnd={value => saved.set(value)} data-testid="brightness" />
      <text role="status" className="text-footnote text-muted">{`Saved at ${saved.value}%`}</text>
    </view>
  );
}
```

### A position in a recording, with what has loaded

```tsx
import { useLocal } from '@lingxia/view';
import { Slider } from '@liulidesign/core/Slider';

/** A position in a recording: the label is hidden, the times stay visible, the loaded part is quieter. */
export function Position() {
  const at = useLocal('at', 84);
  const time = (seconds: number) => `${Math.floor(seconds / 60)}:${String(seconds % 60).padStart(2, '0')}`;
  return (
    <view className="gap-1">
      <text className="text-body font-medium text-text">Interview with the architect</text>
      <Slider bind={at} label="Position" isLabelHidden valueLabel={`${time(at.value)} of 3:10`} max={190} secondaryValue={132} size="sm" data-testid="position" />
    </view>
  );
}
```

### Small and medium

```tsx
import { Held } from '../example';
import { Slider } from '@liulidesign/core/Slider';

export function Sizes() {
  return (
    <view className="gap-4">
      <Held value={30}>{(level, set) => <Slider value={level} onChange={set} label="Small" size="sm" />}</Held>
      <Held value={60}>{(level, set) => <Slider value={level} onChange={set} label="Medium" />}</Held>
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
| `valueLabel` | `string` |  | The value in words, drawn at the end of the label row and read as the value: "40%", "1:24 of 3:10". Liuli formats no numbers. |
| `secondaryValue` | `number` |  | A second, quieter range from `min`, such as how much of a recording has loaded. |
| `description` | `string` |  | Help text under the slider, read as its description. |
| `min` | `number` | `0` |  |
| `max` | `number` | `100` |  |
| `step` | `number` | `1` | The value snaps to it. |
| `marks` | `readonly SliderMark[]` |  | Ticks under the track, with optional labels, in ascending order: the ends of the scale, or each step of a short one. |
| `onChangeEnd` | `((value: number) => void)` |  | Runs once when a drag ends, with the value it ended on: for work too heavy to do at every step, such as saving. |
| `size` | `SliderSize` | `'md'` | `sm` for dense rows and media controls. |
| `variant` | `SliderVariant` | `'standalone'` | `row` sits in a list group among `Item` rows: the row's padding, the Item label style, the value at the end in muted. |
| `isDisabled` | `boolean` | `false` |  |
| `className` | `string` |  |  |
| `data-testid` | `string` |  | On the slider. |

## Anatomy

| Part | Element | Required | Description |
| --- | --- | --- | --- |
| label | `<text>` | yes | The visible label; visually hidden with `isLabelHidden`, still the name. |
| value | `<text>` | no | The value in words, from `valueLabel`; read as the slider's value. |
| control | `<slider>` | yes | The slider primitive: track, range, the optional secondary range, the thumb, and the accessible node. |
| description | `<text>` | no | Help text, read as the description. |

## What an app may override

- `className`: classes merged after the recipe's, so they win.
- The recipe, `sliderRecipe` from `@liulidesign/core/Slider/recipe`: the classes of each part, to style an element of your own to match. The component does not read a changed copy.
- Scale tokens it reads, set in the app's stylesheet under `@theme`: `--spacing-control-sm`, `--spacing-control-md`, `--text-caption`, `--text-footnote`, `--text-body`, `--shadow-contact`.
- Colour roles it paints with, set by the theme pack: `accent`, `focus`, `knob`, `line`, `line-strong`, `text`, `text-faint`, `text-muted`.

## Accessibility

- Pattern: slider.
- Name: label.
- Description: description.
- States: disabled (from isDisabled).
- Left / Down, Right / Up: One step down or up..
- Home, End: The minimum or the maximum..
- Press target: sm 32 px, md 40 px.
- `valueLabel` is the spoken value: "Large" in place of 4, "1:24 of 3:10" in place of 84.
- The press area is the whole control, 40 px tall (32 at `sm`), whatever the thumb's drawn size.
- The empty track and the secondary range are decorative and have no contrast minimum: the value is carried by the range and the thumb.
- The thumb is a `knob` disc, white in both appearances, with an accent rim, so it reads on the range, the track and a dark page alike. On the solid dark accent it would fall to 2.72:1, so the range is the accent at 80 %.
- Disabled, the range keeps its accent at reduced strength, so the value still shows as the filled part.

| Contrast | Pair | Minimum | Light | Dark |
| --- | --- | --- | --- | --- |
| filled range against the surface | `surface` on `accent/80` over `surface` | 3:1 | 3.81:1 | 4.45:1 |
| thumb rim against the track | `accent` on `line` | 3:1 | 4.29:1 | 4.45:1 |
| thumb against the range | `knob` on `accent/80` over `surface` | 3:1 | 3.81:1 | 3.78:1 |
| thumb against the range, straight on the page | `knob` on `accent/80` over `canvas` | 3:1 | 3.88:1 | 3.89:1 |
| keyboard focus ring | `focus` on `surface` | 3:1 | 5.62:1 | 6.19:1 |

Ratios are computed from @lingxia/view/theme and rounded down.

## Guidance

- Do: Show the value in words when the number alone means little: "Large", "1:24 of 3:10".
- Do: Pick a step the person can hit with a finger: at most a hundred stops along the track.
- Don't: Use a slider for an exact number the person knows; use a text input.
- Don't: Act on every change when the work is slow; apply when the value settles.

## Not yet

| Missing | Needs | Meanwhile |
| --- | --- | --- |
| A tone for use over a picture, sizing of the control box, and a forced left-to-right direction. | a media tone and part-level layout props on Slider | VideoControls builds its scrubber on `<slider>` directly. |

Related: ProgressBar.
