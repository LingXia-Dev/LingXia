<!-- Generated from the doc file by scripts/generate.mjs; do not edit. -->

# TimeInput

A labelled time field that opens an hour and minute picker. Status: preview.

```ts
import { TimeInput } from '@liulidesign/core/TimeInput';
```

TimeInput takes a time of day: a reminder, an alarm, the start of quiet hours. A tap opens the View's time picker, in the user's 12- or 24-hour clock; `step` sets the minutes offered. It has Field's label, help text and status. The value is `HH:MM` (or `HH:MM:SS`), or `''` while empty.

## Examples

### A daily reminder

```tsx
import { useLocal } from '@lingxia/view';
import { TimeInput } from '@liulidesign/core/TimeInput';

/** A daily reminder, bound to a View-local cell. */
export function Reminder() {
  const at = useLocal('at', '09:00');
  return (
    <view className="gap-2">
      <TimeInput bind={at} label="Remind me at" description="Every weekday." data-testid="remind" />
      <text className="text-footnote text-muted" data-testid="remind-value">{`Set for ${at.value}`}</text>
    </view>
  );
}
```

### Every 15 minutes, within the evening

```tsx
import { useLocal } from '@lingxia/view';
import { TimeInput } from '@liulidesign/core/TimeInput';

/** Quiet hours: every 15 minutes, within the evening. */
export function QuietHours() {
  const from = useLocal('from', '22:00');
  return <TimeInput bind={from} label="Quiet from" step={900} min="18:00" max="23:45" data-testid="quiet" />;
}
```

### Sizes

```tsx
import { Held } from '../example';
import { TimeInput } from '@liulidesign/core/TimeInput';

export function Sizes() {
  return (
    <view className="gap-3">
      <Held value="09:30">{(time, set) => <TimeInput label="Small" size="sm" value={time} onChange={set} />}</Held>
      <Held value="09:30">{(time, set) => <TimeInput label="Medium" value={time} onChange={set} />}</Held>
      <Held value="09:30">{(time, set) => <TimeInput label="Large" size="lg" value={time} onChange={set} />}</Held>
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
| `bind` | `Local<string>` |  | A View-local cell the field reads and writes; use it or `value` with `onChange`. |
| `value` | `string` |  | The app's value; pass `onChange` with it. |
| `onChange` | `((value: string) => void)` |  | Runs with the value picked; the app passes it back as `value`. |
| `placeholder` | `string` |  | Shown while empty, such as "Choose a time"; never a substitute for `label`. |
| `min` | `string` |  | The earliest time offered, `HH:MM`. |
| `max` | `string` |  | The latest time offered, `HH:MM`. |
| `step` | `number` | `60` | Seconds between the times offered: 300 offers every 5 minutes. |
| `onSubmit` | `((value: string) => void)` |  | Runs with the time picked, after `onChange`. |
| `className` | `string` |  | Classes for the field column. |
| `controlClassName` | `string` |  | Classes for the bordered box. |
| `data-testid` | `string` |  | On the editor. |

## Anatomy

| Part | Element | Required | Description |
| --- | --- | --- | --- |
| box | `<view>` | yes | TextInput's bordered box; it takes the focus colour while the field is focused and the status colour otherwise. |
| input | `<input>` | yes | The time editor; the time shows in the user's clock, and a tap opens the picker. |
| icon | `<vector>` | yes | A clock mark at the end that grows with the text, hidden from assistive tech. |

## What an app may override

- `className`: Classes for the field column.
- `controlClassName`: Classes for the bordered box.
- The recipe, `timeInputRecipe` from `@liulidesign/core/TimeInput/recipe`: the classes of each part, to style an element of your own to match. The component does not read a changed copy.
- Scale tokens it reads, set in the app's stylesheet under `@theme`: `--spacing-control-sm`, `--spacing-control-md`, `--spacing-control-lg`, `--text-footnote`, `--text-body`, `--text-callout`, `--radius-control`.
- Colour roles it paints with, set by the theme pack: `accent`, `danger`, `line-strong`, `surface`, `text`, `text-faint`, `text-muted`, `warning`.

## Accessibility

- Pattern: textbox.
- Name: label.
- Description: description, or disabledMessage while disabled, then the status message.
- States: disabled (from isDisabled); invalid (from status.type === 'error'); required (from isRequired).
- Enter, Space: Opens the picker..
- Escape: Closes the picker without a change..
- Return: In the picker, picks; then onSubmit runs..
- Digits: In the picker, fill HH:MM..
- Press target: sm 32 px, md 40 px, lg 48 px.
- The picker is the View's own, drawn in the theme's roles: a popover under the field with a pointer, a sheet with Cancel and Done on touch.
- The picker follows the user's 12- or 24-hour setting; the value is always 24-hour.
- Sizes sm (32) and md (40) are shorter than the 44 pt touch minimum; a finger still reaches them in a 44 pt band around the box, which the host adds without changing what is drawn.

| Contrast | Pair | Minimum | Light | Dark |
| --- | --- | --- | --- | --- |
| value | `text` on `surface` | 4.5:1 | 16.26:1 | 13.87:1 |
| placeholder | `text-muted` on `surface` | 4.5:1 | 5.89:1 | 6.64:1 |
| box boundary | `line-strong` on `surface` | 3:1 | 3.47:1 | 3.53:1 |
| end mark | `text-muted` on `surface` | 3:1 | 5.89:1 | 6.64:1 |

Ratios are computed from @lingxia/view/theme and rounded down.

## Guidance

- Do: Set `step` to the precision people need: 300 or 900 seconds for most schedules.
- Do: Say in the description what the time repeats on, if it does.
- Don't: Use it for a duration; a time of day is not an amount of time.
- Don't: Use it for a date and time together without a DateInput beside it.

Related: DateInput, TextInput, Field.
