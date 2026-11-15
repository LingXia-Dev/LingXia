import { cx, useLocal } from '@lingxia/view';
import { IconButton } from '../IconButton/IconButton';
import { calendarRecipe as r } from './Calendar.recipe';
import { isDate, parts, shiftMonth, toDate, weekdayOf, weeksOf, type CalendarDate } from './dates';

export type { CalendarDate } from './dates';

/** The words a calendar shows and speaks; `@liulidesign/core/Calendar/en` and `/zh` are ready made. */
export interface CalendarLocale {
  /** The day a week starts on: 0 for Sunday, 1 for Monday. */
  firstDay: number;
  /** Seven short weekday names, Sunday first: "S", "M", … */
  weekdays: readonly string[];
  /** The month's title: "November 2026". `month` is 1 to 12. */
  monthTitle: (year: number, month: number) => string;
  /** A day's accessible name: "Sunday 15 November 2026", saying so when it is today. `weekday` is 0 for Sunday. */
  dayLabel: (year: number, month: number, day: number, weekday: number, isToday: boolean) => string;
  previousMonth: string;
  nextMonth: string;
}

export interface CalendarRange {
  start: CalendarDate | null;
  end: CalendarDate | null;
}

export type CalendarSelection =
  | {
      /** The chosen day, or `null`. */
      value: CalendarDate | null;
      /** Runs with the pressed day. */
      onChange: (value: CalendarDate) => void;
      range?: never;
      onRangeChange?: never;
    }
  | {
      /** The chosen span. A first press sets `start` and clears `end`; a second sets `end`, the earlier day first. */
      range: CalendarRange;
      onRangeChange: (range: CalendarRange) => void;
      value?: never;
      onChange?: never;
    };

export type CalendarProps = CalendarSelection & {
  /** Names the calendar for assistive tech: "Due date", "Stay". */
  label: string;
  locale: CalendarLocale;
  /** The month shown, as `YYYY-MM`; pass it with `onMonthChange` to control it. By default the chosen day's month, else today's. */
  month?: string;
  onMonthChange?: (month: string) => void;
  /** The earliest and latest day that can be chosen. */
  min?: CalendarDate;
  max?: CalendarDate;
  /** Days that cannot be chosen: closed days, taken days. */
  isDateDisabled?: (date: CalendarDate) => boolean;
  /** The day marked as today, and the month shown when nothing is chosen. A View takes the time from its page, not from a clock. */
  today: CalendarDate;
  className?: string;
  /** On the calendar; a day is `<id>-<YYYY-MM-DD>`, the month buttons `<id>-previous` and `<id>-next`. */
  'data-testid'?: string;
};

/** A day's weekday, 0 for Sunday to 6 for Saturday: for an `isDateDisabled` that rules out weekends. */
export const calendarWeekday = (date: CalendarDate) => weekdayOf(...parts(date));

/** A month of days to choose one day or a span from, drawn in the page. */
export function Calendar(props: CalendarProps) {
  const { label, locale, min, max, isDateDisabled, className, 'data-testid': testId } = props;
  const now = props.today;
  const anchor = props.range ? props.range.start : props.value;
  const own = useLocal('month', (isDate(anchor) ? anchor : now).slice(0, 7));
  const shown = props.month ?? own.value;
  const [year, month] = [Number(shown.slice(0, 4)), Number(shown.slice(5, 7))];
  const go = (by: number) => {
    const [nextYear, nextMonth] = shiftMonth(year, month, by);
    const next = toDate(nextYear, nextMonth, 1).slice(0, 7);
    own.set(next);
    props.onMonthChange?.(next);
  };
  const [start, end] = props.range ? [props.range.start, props.range.end] : [props.value, props.value];
  const press = (date: CalendarDate) => {
    if (!props.range) return props.onChange(date);
    const { start: from, end: to } = props.range;
    // A span in progress is finished, the earlier day first; a finished or empty one starts over.
    if (from !== null && to === null) return props.onRangeChange(date < from ? { start: date, end: from } : { start: from, end: date });
    return props.onRangeChange({ start: date, end: null });
  };
  const ordered = Array.from({ length: 7 }, (_, index) => locale.weekdays[(locale.firstDay + index) % 7]);
  const isOff = (date: CalendarDate) => (min !== undefined && date < min) || (max !== undefined && date > max) || isDateDisabled?.(date) === true;
  const [previous, following] = [shiftMonth(year, month, -1), shiftMonth(year, month, 1)];
  return (
    <view role="group" aria-label={label} data-testid={testId} className={cx(r.root.base, className)}>
      <view className={r.header.base}>
        <text role="status" className={r.title.base}>{locale.monthTitle(year, month)}</text>
        <IconButton
          label={locale.previousMonth}
          icon="chevron-start"
          size="sm"
          isDisabled={min !== undefined && toDate(previous[0], previous[1], 31) < min}
          onClick={() => go(-1)}
          data-testid={testId && `${testId}-previous`}
        />
        <IconButton
          label={locale.nextMonth}
          icon="chevron-end"
          size="sm"
          isDisabled={max !== undefined && toDate(following[0], following[1], 1) > max}
          onClick={() => go(1)}
          data-testid={testId && `${testId}-next`}
        />
      </view>
      <view role="grid" aria-label={locale.monthTitle(year, month)} className={r.grid.base}>
        <view aria-hidden className={r.week.base}>
          {ordered.map((name, index) => <text key={String(index)} className={r.weekday.base}>{name}</text>)}
        </view>
        {weeksOf(year, month, locale.firstDay).map((week, row) => (
          <view key={String(row)} role="row" className={r.week.base}>
            {week.map((day, column) => {
              if (day === null) return <view key={`blank-${column}`} className={r.slot.base} />;
              const date = toDate(year, month, day);
              const isStart = date === start;
              const isEnd = date === end;
              const isChosen = isStart || isEnd;
              const isSpan = start !== null && end !== null && start !== end;
              const isBetween = isSpan && date > start && date < end;
              return (
                <view key={date} className={cx(r.slot.base, isBetween && r.slot.between, isSpan && isStart && r.slot.start, isSpan && isEnd && r.slot.end)}>
                  <button
                    role="gridcell"
                    aria-label={locale.dayLabel(year, month, day, weekdayOf(year, month, day), date === now)}
                    aria-selected={isChosen || isBetween}
                    disabled={isOff(date)}
                    onClick={() => press(date)}
                    data-testid={testId && `${testId}-${date}`}
                    className={cx(r.day.base, isChosen ? r.day.chosen : r.day.idle, !isChosen && date === now && r.day.today)}
                  >
                    <text className={cx(r.dayLabel.base, isChosen && r.dayLabel.chosen)}>{String(day)}</text>
                  </button>
                </view>
              );
            })}
          </view>
        ))}
      </view>
    </view>
  );
}
