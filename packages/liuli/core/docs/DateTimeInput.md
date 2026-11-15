<!-- Generated from the doc file by scripts/generate.mjs; do not edit. -->

# DateTimeInput

A day and a time of day as one value. Status: preview.

```ts
import { DateTimeInput } from '@liulidesign/core/DateTimeInput';
```

DateTimeInput puts a DateInput and a TimeInput side by side under one label, help text and status, and gives the app one value, `YYYY-MM-DDTHH:mm`, once both parts are set. Each part opens the platform's own picker. The value is in the user's own time; the app converts it.

## Examples

### A reminder

```tsx
import { useLocal } from '@lingxia/view';
import { DateTimeInput } from '@liulidesign/core/DateTimeInput';

/** When a reminder goes off: one value, two parts. */
export function Reminder() {
  const when = useLocal('when', '2026-11-20T09:00');
  return (
    <view className="gap-2">
      <DateTimeInput bind={when} label="Remind me" dateLabel="Reminder date" timeLabel="Reminder time" description="In your own time zone." data-testid="remind" />
      <text role="status" className="text-footnote text-muted">{when.value === '' ? 'No reminder set' : `Set for ${when.value.replace('T', ' at ')}`}</text>
    </view>
  );
}
```

### Required, with an error

```tsx
import { useLocal } from '@lingxia/view';
import { DateTimeInput } from '@liulidesign/core/DateTimeInput';

/** Empty and required, after a send with no time. */
export function Required() {
  const starts = useLocal('starts', '');
  return (
    <DateTimeInput
      bind={starts}
      label="Starts"
      dateLabel="Start date"
      timeLabel="Start time"
      isRequired
      minDate="2026-11-15"
      status={starts.value === '' ? { type: 'error', message: 'Choose a day and a time.' } : undefined}
      data-testid="starts"
    />
  );
}
```

### In a narrow place

```tsx
import { useLocal } from '@lingxia/view';
import { DateTimeInput } from '@liulidesign/core/DateTimeInput';

/** In a narrow place, such as a side panel, the time goes under the date and both fill the width. */
export function Narrow() {
  const when = useLocal('due', '2026-12-01T17:30');
  return (
    <view className="w-56">
      <DateTimeInput bind={when} label="Due" dateLabel="Due date" timeLabel="Due time" data-testid="due" />
    </view>
  );
}
```

## Props

| Prop | Type | Default | Description |
| --- | --- | --- | --- |
| `label` (required) | `string` |  | The visible label and the control's accessible name. |
| `isLabelHidden` | `boolean` | `false` | Hides the visible label; the control keeps it as its name. |
| `description` | `string` |  | Help text under the control, read as the control's description. |
| `status` | `FieldStatus` |  | An `error` status marks the control invalid; its message is read as the control's description. |
| `isRequired` | `boolean` | `false` | Marks the label with an asterisk and the control as required. Validation is the app's. |
| `hasRequiredMark` | `boolean` | `true` | Draws the asterisk on a required label; turn it off on a form where every field is required. |
| `optionalLabel` | `string` |  | A word after the label for a field that may be left empty, as the app writes it: "optional". Read before the help text. |
| `info` | `FieldInfo` |  | An info button after the label that opens a note. For what most people need, use `description`: it is always in view. |
| `isDisabled` | `boolean` | `false` |  |
| `disabledMessage` | `string` |  | Shown in place of `description` while disabled. |
| `size` | `FieldSize` | `'md'` |  |
| `bind` | `Local<string>` |  | A View-local cell holding `YYYY-MM-DDTHH:mm`, or `''` until both parts are set. |
| `value` | `string` |  | `YYYY-MM-DDTHH:mm` in the user's own time, or `''`. |
| `onChange` | `((value: string) => void)` |  | Runs with the whole value once both parts are set, and with `''` when either is cleared. |
| `dateLabel` (required) | `string` |  | The date part's accessible name: "Start date". |
| `timeLabel` (required) | `string` |  | The time part's accessible name: "Start time". |
| `minDate` | `string` |  | The earliest day, as `YYYY-MM-DD`. |
| `maxDate` | `string` |  | The latest day, as `YYYY-MM-DD`. |
| `timeStep` | `number` | `60` | The time's step in seconds. |
| `className` | `string` |  |  |
| `data-testid` | `string` |  | On the group; the parts get `<testid>-date` and `<testid>-time`. |

## Anatomy

| Part | Element | Required | Description |
| --- | --- | --- | --- |
| group | `<view>` | yes | The two parts in a row that wraps: a group named by the label. |
| date | `<DateInput>` | yes | A DateInput named by `dateLabel`; it takes the width left. |
| time | `<TimeInput>` | yes | A TimeInput named by `timeLabel`. |

## Accessibility

- Pattern: none.
- Name: label.
- Description: description, then status.
- States: undefined (from undefined).
- Tab: To the date, then to the time.
- Enter or Space: Opens the part's picker.
- Press target: .
- A group named by the field's label and described by its help and status; each part has its own name.
- An error marks both parts invalid and is said once, under the pair.

| Contrast | Pair | Minimum | Light | Dark |
| --- | --- | --- | --- | --- |

Ratios are computed from @lingxia/view/theme and rounded down.

## Guidance

- Do: Name the parts in full: "Start date", "Start time".
- Do: Use it when the day and the time are one answer: a reminder, a meeting, a deadline.
- Don't: Use it when only the day matters; that is a DateInput.
- Don't: Show a time zone inside it; say which zone in the help text.

## Not yet

| Missing | Needs | Meanwhile |
| --- | --- | --- |
| undefined | undefined | undefined |

Related: DateInput, TimeInput, DateRangeInput, Timestamp.
