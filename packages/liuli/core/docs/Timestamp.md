<!-- Generated from the doc file by scripts/generate.mjs; do not edit. -->

# Timestamp

When something happened, in the words people use. Status: preview.

```ts
import { formatTimestamp, Timestamp } from '@liulidesign/core/Timestamp';
```

Timestamp writes a moment as people say it: "12 minutes ago" for today, "Yesterday, 18:30", then the date, with the year only when it is another year. It takes the moment and the present as local date-times from the app, in the page's language: Liuli ships English and Chinese, and an lxapp changes a phrase by naming its `@liuli.timestamp.*` key. The time follows the user's 12- or 24-hour clock.

## Examples

### An activity list

```tsx
import { Avatar } from '@liulidesign/core/Avatar';
import { Timestamp } from '@liulidesign/core/Timestamp';

const now = '2026-11-14T10:05';
const activity = [
  { who: 'Ana Lima', what: 'commented on Trip to the coast', when: '2026-11-14T10:05' },
  { who: 'Kai Chen', what: 'shared Reading list with you', when: '2026-11-14T09:42' },
  { who: 'Mira Patel', what: 'edited Ideas for the garden', when: '2026-11-14T07:10' },
  { who: 'Sam Ortiz', what: 'archived Gift list', when: '2026-11-13T18:30' },
  { who: 'Tomás Reis', what: 'joined the workspace', when: '2026-11-02T11:15' },
  { who: 'Ana Lima', what: 'created the workspace', when: '2025-12-19T16:00' },
] as const;

/** An activity list: how long ago for today, then yesterday, then the date. */
export function Activity() {
  return (
    <view className="gap-3">
      {activity.map((entry, index) => (
        <view key={entry.when + entry.who} className="flex-row items-center gap-3">
          <Avatar name={entry.who} size="sm" isDecorative />
          <view className="grow basis-0 min-w-0 gap-0.5">
            <text className="text-body text-text">{`${entry.who} ${entry.what}`}</text>
            <Timestamp value={entry.when} now={now} className="text-footnote text-muted" data-testid={`when-${index}`} />
          </view>
        </view>
      ))}
    </view>
  );
}
```

### The four forms

```tsx
import { Timestamp } from '@liulidesign/core/Timestamp';

const now = '2026-11-14T10:05';

/** The same moment in each form. */
export function Formats() {
  const value = '2026-11-13T18:30';
  return (
    <view className="gap-2">
      {(['relative', 'date', 'time', 'date-time'] as const).map(format => (
        <view key={format} className="flex-row items-baseline justify-between gap-4">
          <text className="text-footnote text-muted">{format}</text>
          <Timestamp value={value} now={now} format={format} className="text-body text-text" data-testid={`format-${format}`} />
        </view>
      ))}
    </view>
  );
}
```

## Props

| Prop | Type | Default | Description |
| --- | --- | --- | --- |
| `value` (required) | `string` |  | The moment, in the user's own time. |
| `now` (required) | `string` |  | The present, in the same form. The app passes it and refreshes it; Liuli has no clock. |
| `format` | `TimestampFormat` | `'relative'` | `relative` says how long ago for today, "yesterday" with the time for yesterday, and the date before that. |
| `className` | `string` |  |  |
| `data-testid` | `string` |  |  |

## Anatomy

| Part | Element | Required | Description |
| --- | --- | --- | --- |
| root | `<text>` | yes | The text; its size and colour come from `className`. |

## What an app may override

- `className`: classes merged after the recipe's, so they win.
- The recipe, `timestampRecipe` from `@liulidesign/core/Timestamp/recipe`: the classes of each part, to style an element of your own to match. The component does not read a changed copy.
- It reads no Liuli scale token.
- It paints with no colour role of its own.

## Accessibility

- Pattern: none.
- Name: none; its content is read.
- Description: none.
- Press target: .
- It is read in full, as the date and the time, whatever short form is shown.
- It does not update itself and is not a live region; the app passes a new `now` when it wants the words to move on.

| Contrast | Pair | Minimum | Light | Dark |
| --- | --- | --- | --- | --- |
| in the muted colour | `text-muted` on `surface` | 4.5:1 | 5.89:1 | 6.64:1 |

Ratios are computed from @lingxia/view/theme and rounded down.

## Guidance

- Do: Use `relative` in lists of recent things; use `date-time` where the exact moment matters, such as a receipt.
- Do: Refresh `now` when the screen comes back into view, and about once a minute while it is shown.
- Don't: Use it for a length of time; that is Timer.
- Don't: Pass a time in another zone; convert it to the user's own time first.

## Limits

- There is no clock and no time zone in the View: the app passes `now` and local date-times.
- Words follow the page's language; Liuli ships English and Chinese.

Related: Timer, Calendar, DateInput.
