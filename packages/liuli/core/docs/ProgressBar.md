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
      <Spinner label="Loading" size="sm" />
      <Spinner label="Loading messages" data-testid="spinner" />
      <Spinner label="Loading" size="lg" />
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
| `className` | `string` |  |  |
| `data-testid` | `string` |  | On the progress bar. |

## Spinner props

| Prop | Type | Default | Description |
| --- | --- | --- | --- |
| `label` (required) | `string` |  | The accessible name: what is loading. Not drawn. |
| `size` | `SpinnerSize` | `'md'` |  |
| `className` | `string` |  |  |
| `data-testid` | `string` |  |  |

## Anatomy

| Part | Element | Required | Description |
| --- | --- | --- | --- |
| header | `<view>` | no | The label row: `label`, then `valueLabel`; hidden from assistive tech, since the bar carries both. |
| track | `<view>` | yes | The bar itself, and the element assistive tech reports as a progress bar. |
| fill | `<view>` | yes | The amount done, as a share of the track width; a sweeping segment while the amount is unknown. |
| spinner | `<vector>` | no | Spinner's spinning arc. |

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
| fill against the track | `accent` on `surface-sunken` | 3:1 | needs the resolved palette | needs the resolved palette |
| fill and spinner on a card | `accent` on `surface` | 3:1 | needs the resolved palette | needs the resolved palette |
| label | `text` on `surface` | 4.5:1 | 16.26:1 | 13.87:1 |
| value | `text-muted` on `surface` | 4.5:1 | 5.89:1 | 6.64:1 |

Ratios are computed from crates/lingxia-theme/packs/default.json and rounded down.

## Guidance

- Do: Name what is in progress in `label`: "Uploading photos", not "Progress".
- Do: Pass `valueLabel` in the app's own words and units; Liuli formats no numbers.
- Do: Use Spinner for waits under a few seconds, and ProgressBar when the amount is known or the wait is long.
- Don't: Show a ProgressBar and a Spinner for the same task.
- Don't: Use ProgressBar as a gauge the user sets; that is a slider.
- Don't: Put a Spinner inside a Button; Button has its own loading state.

## Not yet

| Missing | Needs | Meanwhile |
| --- | --- | --- |
| Finding a progress bar by role in device tests. | progressbar in the test locator roles | Device checks find it by `data-testid`; Node tests read its role. |

Related: Button.
