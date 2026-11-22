<!-- Generated from the doc file by scripts/generate.mjs; do not edit. -->

# Calendar

A month of days in the page, to choose one day or a span. Status: preview.

```ts
import { Calendar } from '@liulidesign/core/Calendar';
```

Calendar draws a month in the page and lets people press a day, or two days for a span: a due date beside its task, the nights of a stay. Days are `YYYY-MM-DD` text. The chosen day is a filled circle, a span is a soft band between its ends, and today is ringed. Its words follow the page's language and its weeks start on the user's first weekday. For a date typed into a form, DateInput opens the platform's own picker.

## Examples

### One day

```tsx
import { useLocal } from '@lingxia/view';
import { Calendar } from '@liulidesign/core/Calendar';

const today = '2026-11-15';

/** One day: the chosen day is filled, today is ringed. */
export function Day() {
  const day = useLocal<string | null>('day', '2026-11-19');
  return <Calendar label="Due date" value={day.value} onChange={day.set} today={today} data-testid="due" />;
}
```

### A span of days

```tsx
import { useLocal } from '@lingxia/view';
import { Calendar, type CalendarRange } from '@liulidesign/core/Calendar';

const today = '2026-11-15';

/** A span: the first press starts it, the second ends it. Days before today cannot be chosen. */
export function Stay() {
  const stay = useLocal<CalendarRange>('stay', { start: '2026-11-18', end: '2026-11-24' });
  return (
    <view className="gap-2">
      <Calendar label="Stay" isRange value={stay.value} onChange={stay.set} min={today} today={today} data-testid="stay" />
      <text role="status" className="px-2 text-footnote text-muted tabular-nums">{`${stay.value.start ?? ''} – ${stay.value.end ?? ''}`}</text>
    </view>
  );
}
```

### Days ruled out

```tsx
import { useLocal } from '@lingxia/view';
import { Calendar, calendarWeekday } from '@liulidesign/core/Calendar';

const today = '2026-11-15';
const closed = (date: string) => calendarWeekday(date) % 6 === 0;

/** Days the app rules out stay in the grid, dimmed: here the weekends. */
export function Weekdays() {
  const day = useLocal<string | null>('day', null);
  return <Calendar label="Appointment" value={day.value} onChange={day.set} isDateDisabled={closed} today={today} month="2026-12" data-testid="appointment" />;
}
```

## Props

| Prop | Type | Default | Description |
| --- | --- | --- | --- |
| `isRange` | `boolean` | `false` | Chooses one day. Chooses a span of days. |
| `value` (required) | `string \| CalendarRange \| null` |  | The chosen day, or `null`. The chosen span. A first press sets `start` and clears `end`; a second sets `end`, the earlier day first. |
| `onChange` (required) | `((value: string) => void) \| ((range: CalendarRange) => void)` |  | Runs with the pressed day. |
| `label` (required) | `string` |  | Names the calendar for assistive tech: "Due date", "Stay". |
| `month` | `string` |  | The month shown, as `YYYY-MM`; pass it with `onMonthChange` to control it. By default the chosen day's month, else today's. |
| `onMonthChange` | `((month: string) => void)` |  |  |
| `min` | `string` |  | The earliest and latest day that can be chosen. |
| `max` | `string` |  |  |
| `isDateDisabled` | `((date: string) => boolean)` |  | Days that cannot be chosen: closed days, taken days. |
| `today` (required) | `string` |  | The day marked as today, and the month shown when nothing is chosen. A View takes the time from its page, not from a clock. |
| `className` | `string` |  |  |
| `data-testid` | `string` |  | On the calendar; a day is `<id>-<YYYY-MM-DD>`, the month buttons `<id>-previous` and `<id>-next`. |

## CalendarRange props

| Prop | Type | Default | Description |
| --- | --- | --- | --- |
| `start` (required) | `string \| null` |  |  |
| `end` (required) | `string \| null` |  |  |

## Anatomy

| Part | Element | Required | Description |
| --- | --- | --- | --- |
| root | `<view>` | yes | The calendar, a group named by `label`. |
| header | `<view>` | yes | The month's title and the buttons to the previous and next month. |
| title | `<text>` | yes | The month and year, announced when the month changes. |
| grid | `<view>` | yes | The weeks, a grid named by the month. |
| weekday | `<text>` | yes | The short weekday names over the columns; hidden from assistive tech, since each day is named in full. |
| slot | `<view>` | yes | A day's seventh of the row; it carries the band of a span. |
| day | `<button>` | yes | A day: a 40 px button, named in full, selected when chosen or inside the span. |

## What an app may override

- `className`: classes merged after the recipe's, so they win.
- The recipe, `calendarRecipe` from `@liulidesign/core/Calendar/recipe`: the classes of each part, to style an element of your own to match. The component does not read a changed copy.
- Scale tokens it reads, set in the app's stylesheet under `@theme`: `--text-caption`, `--text-body`.
- Colour roles it paints with, set by the theme pack: `accent-soft`, `accent-solid`, `accent-solid-hover`, `accent-solid-pressed`, `focus`, `line-strong`, `on-accent-solid`, `text`, `text-muted`.

## Accessibility

- Pattern: none.
- Name: label.
- Description: none.
- States: selected (from value, or range); disabled (from min, max, isDateDisabled).
- Tab: To the month buttons, then into the days as one stop.
- Arrow keys: A day along the week, a week up and down; days that cannot be chosen are passed over.
- Home and End: To the ends of the week.
- PageUp and PageDown: To the month before and after.
- Enter or Space: Chooses the day.
- Press target: md 40 px.
- Each day is named in full, "Thursday 19 November 2026", with "Today" said where it is today, so the weekday row is not read.
- A chosen day, both ends of a span and the days between them report selected.
- The month's title is a status: moving to another month announces it.
- A day that cannot be chosen stays in the grid, dimmed and disabled, so the weeks keep their shape.
- Today is marked by a ring and named in words; the chosen day by a fill. Neither depends on colour alone.

| Contrast | Pair | Minimum | Light | Dark |
| --- | --- | --- | --- | --- |
| day | `text` on `surface` | 4.5:1 | 16.26:1 | 13.87:1 |
| chosen day | `on-accent-solid` on `accent-solid` | 4.5:1 | 5.62:1 | 4.80:1 |
| day inside a span | `text` on `accent-soft` | 4.5:1 | 14.25:1 | 11.26:1 |
| weekday names | `text-muted` on `surface` | 4.5:1 | 5.89:1 | 6.64:1 |
| ring on today | `line-strong` on `surface` | 3:1 | 3.47:1 | 3.53:1 |

Ratios are computed from @lingxia/view/theme and rounded down.

## Guidance

- Do: Use it where the days themselves matter: seeing a span, weekdays against weekends, what is taken.
- Do: Pass `today` from the page's Logic, which knows the time and the person's time zone; a View has no clock.
- Do: Show the chosen span in words beside the calendar, as the app formats dates.
- Do: At large text, a day's circle widens to hold two digits, up to its seventh of the row; give the calendar the full width.
- Don't: Use it for a birth date or any date far from today; DateInput's picker reaches a year faster.
- Don't: Hide days that cannot be chosen; disable them.

## Not yet

| Missing | Needs | Meanwhile |
| --- | --- | --- |
| A field that opens the calendar, for a span typed into a form. | nothing from the View | The calendar sits in the page; put it in a Popover or a BottomSheet. |
| Jumping to a month or year, and two months side by side. | nothing from the View | One month, a month at a time. |
| Month and weekday names in a language Liuli does not translate. | date formatting in the View's engine | English and Chinese; an lxapp adds a language by naming `@liuli.monthName` and `@liuli.weekdayName` in its catalogue. |

Related: DateInput, TimeInput, Popover.
