<!-- Generated from the doc file by scripts/generate.mjs; do not edit. -->

# ProgressBar

How much of a task is done, or a spinner while the amount is unknown. Status: preview.

```ts
import { ProgressBar, Spinner } from '@liulidesign/core/ProgressBar';
```

ProgressBar shows a task that runs for a while: an upload, an export, storage used. Without `value` it sweeps to say the task is running. Spinner, from the same import, is the small form for a wait inside a control or a list. Both are drawn from plain views and vectors, so they look the same on every OS.

## Examples

### An amount the app words

```tsx
import { useLocal } from '@lingxia/view';
import { Button } from '@liulidesign/core/Button';
import { ProgressBar } from '@liulidesign/core/ProgressBar';

/** The app words the amount; Liuli formats no numbers. */
export function Determinate() {
  const sent = useLocal('sent', 3);
  return (
    <view className="gap-3">
      <ProgressBar label="Uploading photos" value={sent.value} max={8} valueLabel={`${sent.value} of 8`} data-testid="upload" />
      <Button
        label="Send one more"
        size="sm"
        width="content"
        isDisabled={sent.value === 8}
        onClick={() => sent.set(sent.value + 1)}
        data-testid="more"
      />
    </view>
  );
}
```

### Running, amount unknown

```tsx
import { ProgressBar } from '@liulidesign/core/ProgressBar';

export function Indeterminate() {
  return <ProgressBar label="Preparing the export" data-testid="export" />;
}
```

### Thin, under its own heading

```tsx
import { ProgressBar } from '@liulidesign/core/ProgressBar';

/** A thin bar under a heading that already says what is happening. */
export function Compact() {
  return (
    <view className="gap-2">
      <text className="text-footnote font-medium text-text">Storage</text>
      <ProgressBar label="Storage used" isLabelHidden size="sm" value={72} valueLabel="7.2 of 10 GB" data-testid="storage" />
    </view>
  );
}
```

### Spinner sizes

```tsx
import { Spinner } from '@liulidesign/core/ProgressBar';

export function Spinners() {
  return (
    <view className="flex-row items-center gap-4">
      <Spinner label="Loading" isLabelHidden size="sm" />
      <Spinner label="Loading messages" isLabelHidden data-testid="spinner" />
      <Spinner label="Loading" isLabelHidden size="lg" />
    </view>
  );
}
```

### A spinner that says what is loading

```tsx
import { Spinner } from '@liulidesign/core/ProgressBar';

/** A whole panel that is loading says what, under the ring. */
export function LoadingMessage() {
  return (
    <view className="items-center py-6">
      <Spinner label="Loading your notes" size="lg" data-testid="loading-notes" />
    </view>
  );
}
```

### Meters in a tone

```tsx
import { ProgressBar } from '@liulidesign/core/ProgressBar';

/** Meters: the fill says how the amount stands, and the words say it too. */
export function Tones() {
  return (
    <view className="gap-4">
      <ProgressBar label="Monthly goal" value={100} valueLabel="Reached" tone="success" data-testid="tone-goal" />
      <ProgressBar label="Storage" value={86} valueLabel="43 of 50 GB, nearly full" tone="warning" data-testid="tone-storage" />
      <ProgressBar label="Data plan" value={100} valueLabel="Over by 1.2 GB" tone="danger" data-testid="tone-plan" />
    </view>
  );
}
```

### A spinner over media, and a quiet one

```tsx
import { Spinner } from '@liulidesign/core/ProgressBar';

/** Over a picture that is still loading, and beside quiet text. */
export function SpinnerTones() {
  return (
    <view className="gap-3">
      <view className="h-36 items-center justify-center rounded-container bg-media">
        <Spinner label="Loading the video" isLabelHidden size="xl" tone="on-media" data-testid="on-media" />
      </view>
      <view className="flex-row items-center gap-2">
        <Spinner label="Checking for updates" isLabelHidden size="sm" tone="muted" data-testid="quiet" />
        <text className="text-footnote text-muted">Checking for updates</text>
      </view>
    </view>
  );
}
```

## Props

| Prop | Type | Default | Description |
| --- | --- | --- | --- |
| `label` (required) | `string` |  | The visible label and the accessible name: what is in progress ("Uploading photos"). |
| `isLabelHidden` | `boolean` | `false` | Hides the label row; `label` stays the name. |
| `value` | `number` |  | The amount done, from 0 to `max`. Leave it out while the amount is unknown. |
| `max` | `number` | `100` |  |
| `valueLabel` | `string` |  | The amount as the app words it ("3 of 8 files"), shown after the label and read in place of a percentage. |
| `size` | `ProgressBarSize` | `'md'` | The track's thickness. |
| `tone` | `ProgressBarTone` | `'accent'` | The fill's colour, for a bar that is a meter: storage, a quota, a goal. Say the state in `valueLabel` too. |
| `className` | `string` |  |  |
| `data-testid` | `string` |  | On the progress bar. |

## Spinner props

| Prop | Type | Default | Description |
| --- | --- | --- | --- |
| `label` (required) | `string` |  | The accessible name: what is loading, drawn under the ring unless `isLabelHidden`. |
| `isLabelHidden` | `boolean` | `false` | Draws the ring alone, the label read only: for a spinner inside a control or a row. Leave the label shown for a page, a panel or a list that has room to say what. |
| `size` | `SpinnerSize` | `'md'` | 16, 24, 32 or 48 px; `xl` for a whole page or a media frame that is loading. |
| `tone` | `SpinnerTone` | `'accent'` |  |
| `className` | `string` |  |  |
| `data-testid` | `string` |  |  |

## Anatomy

| Part | Element | Required | Description |
| --- | --- | --- | --- |
| header | `<view>` | no | The label row: `label`, then `valueLabel`; hidden from assistive tech, since the bar carries both. |
| track | `<view>` | yes | The bar itself, and the element assistive tech reports as a progress bar. |
| fill | `<view>` | yes | The amount done, as a share of the track width; a sweeping segment while the amount is unknown. |
| spinner | `<vector>` | no | Spinner's spinning arc. |
| loading | `<view>` | no | A Spinner with its label shown: the ring over its label, centred; the words are drawn only, the name is read once. |

## What an app may override

- `className`: classes merged after the recipe's, so they win.
- `className` on Spinner: classes merged after the recipe's, so they win.
- The recipe, `progressBarRecipe` from `@liulidesign/core/ProgressBar/recipe`: the classes of each part, to style an element of your own to match. The component does not read a changed copy.
- Scale tokens it reads, set in the app's stylesheet under `@theme`: `--text-footnote`, `--animate-progress-sweep`.
- Colour roles it paints with, set by the theme pack: `accent`, `danger-mark`, `on-media`, `success-mark`, `text`, `text-muted`, `warning-mark`.

## Accessibility

- Pattern: none.
- Name: label.
- Description: none.
- Role progressbar, named by `label`. With `value` it reports the value, 0 and `max`; `valueLabel` is read in place of a percentage.
- Without `value` it reports no value, which assistive tech reads as busy.
- With reduced motion the sweep and the spin stop; the bar and the arc stay drawn.
- Spinner is a progressbar named by its `label`, with nothing drawn but the arc.

| Contrast | Pair | Minimum | Light | Dark |
| --- | --- | --- | --- | --- |
| fill against the track | `accent` on `text/10` over `surface` | 3:1 | 4.62:1 | 4.75:1 |
| fill and spinner on a card | `accent` on `surface` | 3:1 | 5.62:1 | 6.19:1 |
| spinner over media | `on-media` on `media` | 3:1 | 7.94:1 | 7.94:1 |
| muted spinner | `text-muted` on `surface` | 3:1 | 5.89:1 | 6.64:1 |
| a spinner's shown label | `text-muted` on `surface` | 4.5:1 | 5.89:1 | 6.64:1 |
| success fill | `success-mark` on `surface` | 3:1 | 3.71:1 | 7.61:1 |
| warning fill | `warning-mark` on `surface` | 3:1 | 3.70:1 | 8.18:1 |
| danger fill | `danger-mark` on `surface` | 3:1 | 3.70:1 | 5.42:1 |
| label | `text` on `surface` | 4.5:1 | 16.26:1 | 13.87:1 |
| value | `text-muted` on `surface` | 4.5:1 | 5.89:1 | 6.64:1 |

Ratios are computed from @lingxia/view/theme and rounded down.

## Guidance

- Do: Name what is in progress in `label`: "Uploading photos", not "Progress".
- Do: Pass `valueLabel` in the app's own words and units; Liuli formats no numbers.
- Do: Use Spinner for waits under a few seconds, and ProgressBar when the amount is known or the wait is long.
- Don't: Show a ProgressBar and a Spinner for the same task.
- Don't: Use ProgressBar as a gauge the user sets; that is a slider.
- Don't: Put a Spinner inside a Button; Button has its own loading state.

Related: Button.
