<!-- Generated from the doc file by scripts/generate.mjs; do not edit. -->

# Timer

A length of time in steady digits. Status: preview.

```ts
import { formatTimer, Timer } from '@liulidesign/core/Timer';
```

Timer draws a number of seconds as `m:ss`, or `h:mm:ss` from an hour on, in digits that keep their width as they change. It does not count: the app passes `seconds` each time it changes, from Logic or from its own tick.

## Examples

### A countdown

```tsx
import { useLocal } from '@lingxia/view';
import { Button } from '@liulidesign/core/Button';
import { Timer } from '@liulidesign/core/Timer';

/** A countdown the app runs: here each press takes fifteen seconds off. */
export function Countdown() {
  const left = useLocal('left', 125);
  return (
    <view className="items-start gap-3">
      <Timer seconds={left.value} size="lg" aria-label={`${Math.floor(left.value / 60)} minutes ${left.value % 60} seconds left`} data-testid="countdown" />
      <view className="flex-row gap-2">
        <Button label="Skip 15 seconds" onClick={() => left.set(Math.max(0, left.value - 15))} data-testid="skip" />
        <Button label="Reset" variant="ghost" onClick={() => left.set(125)} />
      </view>
    </view>
  );
}
```

### In a line of text

```tsx
import { Timer } from '@liulidesign/core/Timer';

/** In a line of text: how long a code stays valid, how long a call has run. */
export function Inline() {
  return (
    <view className="gap-2">
      <view className="flex-row items-baseline gap-1.5">
        <text className="text-footnote text-muted">Code expires in</text>
        <Timer seconds={272} size="sm" data-testid="expires" />
      </view>
      <view className="flex-row items-baseline gap-1.5">
        <text className="text-body text-text">On a call</text>
        <Timer seconds={4325} data-testid="call" />
      </view>
    </view>
  );
}
```

## Props

| Prop | Type | Default | Description |
| --- | --- | --- | --- |
| `seconds` (required) | `number` |  | The time to show, in seconds: what is left of a countdown, or what has passed. The app counts; Timer draws. |
| `aria-label` | `string` |  | How it is read: "2 minutes 5 seconds left". Without it the digits are read as written. |
| `hasHours` | `boolean` | `false` | Always shows hours, as `0:02:05`. Otherwise hours appear from one hour on. |
| `size` | `TimerSize` | `'md'` |  |
| `className` | `string` |  |  |
| `data-testid` | `string` |  |  |

## Anatomy

| Part | Element | Required | Description |
| --- | --- | --- | --- |
| root | `<text>` | yes | The digits; muted once the time is zero. |

## What an app may override

- `className`: classes merged after the recipe's, so they win.
- The recipe, `timerRecipe` from `@liulidesign/core/Timer/recipe`: the classes of each part, to style an element of your own to match. The component does not read a changed copy.
- Scale tokens it reads, set in the app's stylesheet under `@theme`: `--text-footnote`, `--text-body`, `--text-display`.
- Colour roles it paints with, set by the theme pack: `text`, `text-muted`.

## Accessibility

- Pattern: none.
- Name: aria-label.
- Description: none.
- Press target: .
- Give `aria-label` in words: "2 minutes 5 seconds left". Digits alone are read as a time of day by some screen readers.
- It is not a live region: a countdown read every second is noise. Announce what matters, such as the last ten seconds, with a status of your own.

| Contrast | Pair | Minimum | Light | Dark |
| --- | --- | --- | --- | --- |
| digits | `text` on `surface` | 4.5:1 | 16.26:1 | 13.87:1 |
| digits at zero | `text-muted` on `surface` | 4.5:1 | 5.89:1 | 6.64:1 |

Ratios are computed from @lingxia/view/theme and rounded down.

## Guidance

- Do: Say beside it what the time is: "Code expires in", "Recording".
- Do: Use `hasHours` where the time may pass an hour, so the digits do not jump wider.
- Don't: Use it for a time of day or a date; that is Timestamp.
- Don't: Hide the only way to tell time is running out in the digits; change the screen when it is up.

## Not yet

| Missing | Needs | Meanwhile |
| --- | --- | --- |
| undefined | undefined | undefined |

Related: Timestamp, ProgressBar.
