<!-- Generated from the doc file by scripts/generate.mjs; do not edit. -->

# Calendar

A month of days in the page, to choose one day or a span. Status: preview.

```ts
import { Calendar } from '@liulidesign/core/Calendar';
```

Calendar draws a month in the page and lets people press a day, or two days for a span: a due date beside its task, the nights of a stay. Days are `YYYY-MM-DD` text. The chosen day is a filled circle, a span is a soft band between its ends, and today is ringed. Its words come from a `locale` object; English and Chinese ones are ready to import. For a date typed into a form, DateInput opens the platform's own picker.

## Examples

### One day

```tsx
import { useLocal } from '@lingxia/view';
import { Calendar } from '@liulidesign/core/Calendar';
import { calendarEn } from './locale/en';

const today = '2026-11-15';

/** One day: the chosen day is filled, today is ringed. */
export function Day() {
  const day = useLocal<string | null>('day', '2026-11-19');
  return <Calendar label="Due date" locale={calendarEn} value={day.value} onChange={day.set} today={today} data-testid="due" />;
}
```

### A span of days

```tsx
import { useLocal } from '@lingxia/view';
import { Calendar, type CalendarRange } from '@liulidesign/core/Calendar';
import { calendarEn } from './locale/en';

const today = '2026-11-15';

/** A span: the first press starts it, the second ends it. Days before today cannot be chosen. */
export function Stay() {
  const stay = useLocal<CalendarRange>('stay', { start: '2026-11-18', end: '2026-11-24' });
  return (
    <view className="gap-2">
      <Calendar label="Stay" locale={calendarEn} range={stay.value} onRangeChange={stay.set} min={today} today={today} data-testid="stay" />
      <text role="status" className="px-2 text-footnote text-muted tabular-nums">{`${stay.value.start ?? ''} – ${stay.value.end ?? ''}`}</text>
    </view>
  );
}
```

### Days ruled out

```tsx
import { useLocal } from '@lingxia/view';
import { Calendar, calendarWeekday } from '@liulidesign/core/Calendar';
import { calendarEn } from './locale/en';

const today = '2026-11-15';
const closed = (date: string) => calendarWeekday(date) % 6 === 0;

/** Days the app rules out stay in the grid, dimmed: here the weekends. */
export function Weekdays() {
  const day = useLocal<string | null>('day', null);
  return <Calendar label="Appointment" locale={calendarEn} value={day.value} onChange={day.set} isDateDisabled={closed} today={today} month="2026-12" data-testid="appointment" />;
}
```

### Another language

```tsx
import { useLocal } from '@lingxia/view';
import { Calendar } from '@liulidesign/core/Calendar';
import { calendarZh } from './locale/zh';

const today = '2026-11-15';

/** Another language: the words and the first day of the week come from the locale. */
export function Chinese() {
  const day = useLocal<string | null>('day', '2026-11-19');
  return <Calendar label="截止日期" locale={calendarZh} value={day.value} onChange={day.set} today={today} data-testid="zh" />;
}
```

## Props

| Prop | Type | Default | Description |
| --- | --- | --- | --- |
| `value` | `string \| null` |  | The chosen day, or `null`. |
| `onChange` | `((value: string) => void)` |  | Runs with the pressed day. |
| `range` | `CalendarRange` |  | The chosen span. A first press sets `start` and clears `end`; a second sets `end`, the earlier day first. |
| `onRangeChange` | `((range: CalendarRange) => void)` |  |  |
| `label` (required) | `string` |  | Names the calendar for assistive tech: "Due date", "Stay". |
| `locale` (required) | `CalendarLocale` |  |  |
| `month` | `string` |  | The month shown, as `YYYY-MM`; pass it with `onMonthChange` to control it. By default the chosen day's month, else today's. |
| `onMonthChange` | `((month: string) => void)` |  |  |
| `min` | `string` |  | The earliest and latest day that can be chosen. |
| `max` | `string` |  |  |
| `isDateDisabled` | `((date: string) => boolean)` |  | Days that cannot be chosen: closed days, taken days. |
| `today` (required) | `string` |  | The day marked as today, and the month shown when nothing is chosen. A View takes the time from its page, not from a clock. |
| `className` | `string` |  |  |
| `data-testid` | `string` |  | On the calendar; a day is `<id>-<YYYY-MM-DD>`, the month buttons `<id>-previous` and `<id>-next`. |

## CalendarLocale props

| Prop | Type | Default | Description |
| --- | --- | --- | --- |
| `firstDay` (required) | `number` |  | The day a week starts on: 0 for Sunday, 1 for Monday. |
| `weekdays` (required) | `readonly string[]` |  | Seven short weekday names, Sunday first: "S", "M", … |
| `monthTitle` (required) | `(year: number, month: number) => string` |  | The month's title: "November 2026". `month` is 1 to 12. |
| `dayLabel` (required) | `(year: number, month: number, day: number, weekday: number, isToday: boolean) => string` |  | A day's accessible name: "Sunday 15 November 2026", saying so when it is today. `weekday` is 0 for Sunday. |
| `previousMonth` (required) | `string` |  |  |
| `nextMonth` (required) | `string` |  |  |

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

## Accessibility

- Pattern: none.
- Name: label.
- Description: none.
- States: selected (from value, or range); disabled (from min, max, isDateDisabled).
- Tab: To the month buttons, then through the days.
- Enter or Space: Chooses the day.
- Press target: md 40 px.
- Each day is named in full by the locale, "Thursday 19 November 2026", with "Today" said where it is today, so the weekday row is not read.
- A chosen day, both ends of a span and the days between them report selected.
- The month's title is a status: moving to another month announces it.
- A day that cannot be chosen stays in the grid, dimmed and disabled, so the weeks keep their shape.
- Today is marked by a ring and named in words; the chosen day by a fill. Neither depends on colour alone.

| Contrast | Pair | Minimum | Light | Dark |
| --- | --- | --- | --- | --- |
| day | `text` on `surface` | 4.5:1 | 16.26:1 | 13.87:1 |
| chosen day | `on-accent` on `accent` | 4.5:1 | 5.62:1 | 7.12:1 |
| day inside a span | `text` on `accent-soft` | 4.5:1 | 14.09:1 | 11.26:1 |
| weekday names | `text-muted` on `surface` | 4.5:1 | 5.89:1 | 6.64:1 |
| ring on today | `line-strong` on `surface` | 3:1 | 3.66:1 | 3.69:1 |

Ratios are computed from @lingxia/view/theme and rounded down.

## Guidance

- Do: Use it where the days themselves matter: seeing a span, weekdays against weekends, what is taken.
- Do: Pass `today` from the page's Logic, which knows the time and the person's time zone; a View has no clock.
- Do: Show the chosen span in words beside the calendar, as the app formats dates.
- Do: Spread a ready-made locale to change the first day of the week or a word.
- Don't: Use it for a birth date or any date far from today; DateInput's picker reaches a year faster.
- Don't: Hide days that cannot be chosen; disable them.

## Not yet

| Missing | Needs | Meanwhile |
| --- | --- | --- |
| Arrow keys moving through the days as one Tab stop, a week up and down. | a grid whose cells take one Tab stop and move by arrows in two directions | Every day is its own Tab stop. |
| A field that opens the calendar, for a span typed into a form. | nothing from the View | The calendar sits in the page; put it in a Popover or a BottomSheet. |
| Jumping to a month or year, and two months side by side. | nothing from the View | One month, a month at a time. |
| Month and weekday names from the device's own language. | date formatting in the View's engine | The app passes a `locale`. |

Related: DateInput, TimeInput, Popover.
