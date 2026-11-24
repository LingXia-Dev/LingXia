<!-- Generated from the doc file by scripts/generate.mjs; do not edit. -->

# DateInput

A labelled date field that opens a month picker. Status: preview.

```ts
import { DateInput } from '@liulidesign/core/DateInput';
```

DateInput takes one day: a due date, a birthday, the day a trip starts. A tap opens the View's month picker, which marks today, greys days outside `min` and `max`, and starts weeks on the user's first weekday. It has Field's label, help text and status. The value is `YYYY-MM-DD`, or `''` while empty.

## Examples

### A due date

```tsx
import { useLocal } from '@lingxia/view';
import { DateInput } from '@liulidesign/core/DateInput';

/** A due date, bound to a View-local cell. */
export function DueDate() {
  const due = useLocal('due', '');
  return (
    <view className="gap-2">
      <DateInput bind={due} label="Due date" placeholder="Choose a date" data-testid="due" />
      <text className="text-footnote text-muted" data-testid="due-value">{due.value === '' ? 'No due date' : `Due ${due.value}`}</text>
    </view>
  );
}
```

### A return tied to its departure

```tsx
import { useLocal } from '@lingxia/view';
import { DateInput } from '@liulidesign/core/DateInput';

/** A trip: the return cannot come before the departure. */
export function Trip() {
  const leave = useLocal('leave', '2026-12-18');
  const back = useLocal('back', '2026-12-27');
  const isBefore = back.value !== '' && back.value < leave.value;
  return (
    <view className="gap-4">
      <DateInput bind={leave} label="Leave" min="2026-11-12" data-testid="leave" />
      <DateInput
        bind={back}
        label="Return"
        min={leave.value}
        status={isBefore ? { type: 'error', message: 'Return after you leave.' } : undefined}
        data-testid="back"
      />
    </view>
  );
}
```

### A day and a time in one row

```tsx
import { useLocal } from '@lingxia/view';
import { TimeInput } from '@liulidesign/core/TimeInput';
import { DateInput } from '@liulidesign/core/DateInput';

/** A day and a time sharing a narrow row: each field keeps room for its whole value. */
export function Meeting() {
  const day = useLocal('day', '2026-12-18');
  const at = useLocal('at', '14:30');
  return (
    <view className="flex-row gap-3">
      <DateInput bind={day} label="Day" className="grow basis-0" data-testid="meeting-day" />
      <TimeInput bind={at} label="Starts" className="grow basis-0" data-testid="meeting-time" />
    </view>
  );
}
```

### Sizes

```tsx
import { Held } from '../example';
import { DateInput } from '@liulidesign/core/DateInput';

export function Sizes() {
  return (
    <view className="gap-3">
      <Held value="2026-12-18">{(date, set) => <DateInput label="Small" size="sm" value={date} onChange={set} />}</Held>
      <Held value="2026-12-18">{(date, set) => <DateInput label="Medium" value={date} onChange={set} />}</Held>
      <Held value="2026-12-18">{(date, set) => <DateInput label="Large" size="lg" value={date} onChange={set} />}</Held>
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
| `placeholder` | `string` |  | Shown while empty, such as "Choose a date"; never a substitute for `label`. |
| `min` | `string` |  | The earliest day offered, `YYYY-MM-DD`; earlier days are greyed in the picker. |
| `max` | `string` |  | The latest day offered, `YYYY-MM-DD`. |
| `onSubmit` | `((value: string) => void)` |  | Runs with the day picked, after `onChange`. |
| `className` | `string` |  | Classes for the field column. |
| `controlClassName` | `string` |  | Classes for the bordered box. |
| `data-testid` | `string` |  | On the editor. |

## Anatomy

| Part | Element | Required | Description |
| --- | --- | --- | --- |
| box | `<view>` | yes | TextInput's bordered box; it takes the focus colour while the field is focused and the status colour otherwise. |
| input | `<input>` | yes | The date editor; the day shows in the user's language, and a tap opens the picker. |
| icon | `<vector>` | yes | A calendar mark at the start, before the value in either direction, that grows with the text; hidden from assistive tech. |

## What an app may override

- `className`: Classes for the field column.
- `controlClassName`: Classes for the bordered box.
- The recipe, `dateInputRecipe` from `@liulidesign/core/DateInput/recipe`: the classes of each part, to style an element of your own to match. The component does not read a changed copy.
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
- Arrows, Page Up, Page Down: In the picker, a day, a week, a month..
- Press target: sm 32 px, md 40 px, lg 48 px.
- The picker is the View's own, drawn in the theme's roles: a popover under the field with a pointer, a sheet with Cancel and Done on touch.
- Month and day names follow the display language; the week starts on the user's first weekday.
- Sizes sm (32) and md (40) are shorter than the 44 pt touch minimum; a finger still reaches them in a 44 pt band around the box, which the host adds without changing what is drawn.

| Contrast | Pair | Minimum | Light | Dark |
| --- | --- | --- | --- | --- |
| value | `text` on `surface` | 4.5:1 | 16.26:1 | 13.87:1 |
| placeholder | `text-muted` on `surface` | 4.5:1 | 5.89:1 | 6.64:1 |
| box boundary | `line-strong` on `surface` | 3:1 | 3.47:1 | 3.53:1 |
| start mark | `text-muted` on `surface` | 3:1 | 5.89:1 | 6.64:1 |

Ratios are computed from @lingxia/view/theme and rounded down.

## Guidance

- Do: Bound the range with `min` and `max` when only some days make sense.
- Do: Tie a second date to the first, as a return to its departure, through `min`.
- Don't: Parse the value as local midnight in Logic without a time zone; it is a calendar day.
- Don't: Use it for a time of day; that is TimeInput.

## Limits

- The picker's week starts on the user's own first weekday and takes no override; for a week that is the app's own, show a Calendar or a DateRangeInput with `firstWeekday`.

Related: TimeInput, TextInput, Field.
