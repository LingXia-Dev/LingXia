<!-- Generated from the doc file by scripts/generate.mjs; do not edit. -->

# DateRangeInput

A span of days in a form: a field that opens a calendar. Status: preview.

```ts
import { DateRangeInput } from '@liulidesign/core/DateRangeInput';
```

DateRangeInput asks for a first and a last day: a stay, time off, a report's period. The field shows the span as the app formats it; pressing it opens a Calendar, in a panel under the field for a mouse and in a sheet from the bottom edge for a finger. The first press sets the start, the second the end, and the calendar closes. It takes Field's label, help text and status. For one day, use DateInput.

## Examples

### A span of days

```tsx
import { useLocal } from '@lingxia/view';
import type { CalendarRange } from '@liulidesign/core/Calendar';
import { DateRangeInput } from '@liulidesign/core/DateRangeInput';

const today = '2026-11-15';
const short = (date: string) => `${Number(date.slice(8))} ${months[Number(date.slice(5, 7)) - 1]}`;

/** A span of days in a form: pressing the field opens a calendar, and the second day closes it. */
export function Stay() {
  const stay = useLocal<CalendarRange>('stay', { start: '2026-11-18', end: '2026-11-24' });
  return (
    <DateRangeInput
      label="Stay"
      description="Check-in from 3 pm, check-out by 11 am."
      bind={stay}
      format={short}
      today={today}
      min={today}
      data-testid="stay"
    />
  );
}
```

### Nothing chosen yet

```tsx
import { useLocal } from '@lingxia/view';
import type { CalendarRange } from '@liulidesign/core/Calendar';
import { DateRangeInput } from '@liulidesign/core/DateRangeInput';

const today = '2026-11-15';
const short = (date: string) => `${Number(date.slice(8))} ${months[Number(date.slice(5, 7)) - 1]}`;

/** Nothing chosen yet, and required. */
export function Empty() {
  const leave = useLocal<CalendarRange>('leave', { start: null, end: null });
  return (
    <DateRangeInput
      label="Time off"
      placeholder="First and last day"
      isRequired
      bind={leave}
      format={short}
      today={today}
      data-testid="leave"
    />
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
| `bind` | `Local<CalendarRange>` |  | A View-local cell the control reads and writes; use it or `value` with `onChange`. |
| `value` | `CalendarRange` |  | The chosen span; both ends `null` shows the placeholder. |
| `onChange` | `((range: CalendarRange) => void)` |  | Runs with each press in the calendar: the first day alone, then the finished span. |
| `format` (required) | `(date: string) => string` |  | Turns a day into the text the field shows: "18 Nov". The app formats dates. |
| `today` (required) | `string` |  | Today, from the page's Logic. |
| `placeholder` | `string` |  | Shown while no day is chosen. |
| `min` | `string` |  |  |
| `max` | `string` |  |  |
| `isDateDisabled` | `((date: string) => boolean)` |  |  |
| `firstWeekday` | `Weekday` | `the user's first weekday` | The calendar's first day of the week, as Calendar's `firstWeekday`: 1 is Monday to 7 is Sunday. |
| `hasCloseButton` | `boolean` | `true` | Shows a close button in the calendar's sheet; without it the sheet closes by a tap outside, a drag or Back. |
| `closeLabel` | `string` |  | Names the close button. Defaults to "Close" in the page's language. |
| `className` | `string` |  |  |
| `data-testid` | `string` |  | On the field's button; the calendar is `<id>-calendar`. |

- `bind` | `value` + `onChange`: props from one side only.

## Anatomy

| Part | Element | Required | Description |
| --- | --- | --- | --- |
| value | `<text>` | yes | The span in the field, "18 Nov – 24 Nov", or the placeholder; in tabular figures. |
| icon | `<vector>` | yes | The calendar mark at the field's end, the size of DateInput's. |
| popover | `<view>` | no | With a mouse or trackpad: a panel under the field holding the calendar. |

## What an app may override

- `className`: classes merged after the recipe's, so they win.
- The recipe, `dateRangeInputRecipe` from `@liulidesign/core/DateRangeInput/recipe`: the classes of each part, to style an element of your own to match. The component does not read a changed copy.
- Scale tokens it reads, set in the app's stylesheet under `@theme`: `--shadow-raised`, `--radius-container`.
- Colour roles it paints with, set by the theme pack: `line`, `surface-overlay`, `text-muted`.

## Accessibility

- Pattern: button.
- Name: label.
- Description: the chosen span, then description, then status message.
- States: expanded (from the open sheet); disabled (from isDisabled); modal (from the sheet on touch).
- Enter or Space: On the field: opens the calendar.
- Tab: Through the month buttons and the days.
- Escape: Closes the calendar and keeps what was chosen.
- Press target: sm 32 px, md 40 px, lg 48 px.
- The field is a button named by `label`; the span, then the help text and status, are read as its description.
- The calendar is Calendar: each day is named in full and the span's days report selected.
- On touch the sheet is modal and titled by `label`; a tap outside, a drag down or Back closes it. With a mouse a click outside or Escape closes the panel.
- Closing with only a start chosen keeps that start; the field shows it with an open end.

| Contrast | Pair | Minimum | Light | Dark |
| --- | --- | --- | --- | --- |
| span in the field | `text` on `surface` | 4.5:1 | 16.26:1 | 13.87:1 |
| placeholder | `text-muted` on `surface` | 4.5:1 | 5.89:1 | 6.64:1 |
| field edge | `line-strong` on `surface` | 3:1 | 3.47:1 | 3.53:1 |
| day in the panel | `text` on `surface-overlay` | 4.5:1 | 16.26:1 | 10.11:1 |
| error edge | `danger` on `surface` | 3:1 | 5.21:1 | 5.42:1 |

Ratios are computed from @lingxia/view/theme and rounded down.

## Guidance

- Do: Format days as the rest of the app does, short enough that a span fits the field: "18 Nov", not "Wednesday 18 November 2026".
- Do: Rule out days that cannot be chosen with `min`, `max` and `isDateDisabled`, and say why in the help text.
- Do: Check the finished span in the app (a minimum stay, a taken night) and say what is wrong in `status`.
- Don't: Use it for one day; that is DateInput.
- Don't: Use it for spans of months or years; two Selects or a DateInput each end reach further.

## Not yet

| Missing | Needs | Meanwhile |
| --- | --- | --- |
| Typing the dates into the field. | nothing from the View | The days are pressed in the calendar. |
| Ready-made spans beside the calendar: this week, last month. | nothing from the View | An app puts its own buttons beside the field. |
| Arrow keys through the days. | a grid whose cells take one Tab stop and move by arrows in two directions | Every day is its own Tab stop, as in Calendar. |

Related: Calendar, DateInput, Combobox.
