import { cx, useLocal, useView, type Local, type Weekday } from '@lingxia/view';
import { liuliText, monthName, weekdayInitial, weekdayName } from '../i18n/text';
import { IconButton } from '../IconButton/IconButton';
import { calendarRecipe as r } from './Calendar.recipe';
import { isDate, parts, shiftMonth, toDate, weekdayOf, weeksOf, type CalendarDate } from './dates';
import { dataOf } from '../internal/data';

export type { CalendarDate } from './dates';

/** A day of the week as the environment counts it: 1 is Sunday, 7 is Saturday. */
export type CalendarWeekday = Weekday;

export interface CalendarRange {
  start: CalendarDate | null;
  end: CalendarDate | null;
}

export type CalendarSelection =
  | {
      /** Chooses one day. @default false */
      isRange?: false;
      /** A View-local cell the control reads and writes; use it or `value` with `onChange`. */
      bind: Local<CalendarDate | null>;
      value?: never;
      onChange?: never;
    }
  | {
      isRange?: false;
      bind?: never;
      /** The chosen day, or `null`. */
      value: CalendarDate | null;
      /** Runs with the pressed day. */
      onChange: (value: CalendarDate) => void;
    }
  | {
      /** Chooses a span of days. */
      isRange: true;
      bind: Local<CalendarRange>;
      value?: never;
      onChange?: never;
    }
  | {
      isRange: true;
      bind?: never;
      /** The chosen span. A first press sets `start` and clears `end`; a second sets `end`, the earlier day first. */
      value: CalendarRange;
      onChange: (range: CalendarRange) => void;
    };

export type CalendarProps = CalendarSelection & {
  /** Names the calendar for assistive tech: "Due date", "Stay". */
  label: string;
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
  /** The week's first day, 1 is Monday to 7 is Sunday, for a week that is the app's own (a school timetable). @default the user's first weekday */
  firstWeekday?: CalendarWeekday;
  className?: string;
  /** On the calendar; a day is `<id>-<YYYY-MM-DD>`, the month buttons `<id>-previous` and `<id>-next`. */
  'data-testid'?: string;
};

/** A day's weekday, 1 for Monday to 7 for Sunday: for an `isDateDisabled` that rules out weekends. */
export const calendarWeekday = (date: CalendarDate) => ((weekdayOf(...parts(date)) + 6) % 7 + 1) as CalendarWeekday;

/**
 * A month of days to choose one day or a span from, drawn in the page. Its words follow the page's
 * language and its weeks start on the user's first weekday.
 */
export function Calendar(props: CalendarProps) {
  const { label, min, max, isDateDisabled, className, 'data-testid': testId } = props;
  const userFirst = useView().env.firstWeekday;
  // `weekdayOf` counts 0 = Sunday.
  const firstDay = (props.firstWeekday ?? userFirst ?? 7) % 7;
  const now = props.today;
  const span = props.isRange ? (props.bind ? props.bind.value : props.value) : undefined;
  const day = props.isRange ? null : props.bind ? props.bind.value : props.value;
  const chooseDay = (next: CalendarDate) => {
    if (props.isRange) return;
    if (props.bind) props.bind.set(next);
    else props.onChange(next);
  };
  const chooseSpan = (next: CalendarRange) => {
    if (!props.isRange) return;
    if (props.bind) props.bind.set(next);
    else props.onChange(next);
  };
  const anchor = span ? span.start : day;
  const own = useLocal('month', (isDate(anchor) ? anchor : now).slice(0, 7));
  const shown = props.month ?? own.value;
  const [year, month] = [Number(shown.slice(0, 4)), Number(shown.slice(5, 7))];
  const go = (by: number) => {
    const [nextYear, nextMonth] = shiftMonth(year, month, by);
    const next = toDate(nextYear, nextMonth, 1).slice(0, 7);
    own.set(next);
    props.onMonthChange?.(next);
  };
  const [start, end] = span ? [span.start, span.end] : [day, day];
  const press = (date: CalendarDate) => {
    if (!span) return chooseDay(date);
    const { start: from, end: to } = span;
    // A span in progress is finished, the earlier day first; a finished or empty one starts over.
    if (from !== null && to === null) return chooseSpan(date < from ? { start: date, end: from } : { start: from, end: date });
    return chooseSpan({ start: date, end: null });
  };
  const ordered = Array.from({ length: 7 }, (_, index) => weekdayInitial((firstDay + index) % 7));
  const title = liuliText('@liuli.calendar.title', { monthName: monthName(month), year: String(year) });
  const isOff = (date: CalendarDate) => (min !== undefined && date < min) || (max !== undefined && date > max) || isDateDisabled?.(date) === true;
  const [previous, following] = [shiftMonth(year, month, -1), shiftMonth(year, month, 1)];
  // A month wholly outside the range is not paged to, by the buttons or by a swipe or key on the grid.
  const [hasPrevious, hasNext] = [min === undefined || toDate(previous[0], previous[1], 31) >= min, max === undefined || toDate(following[0], following[1], 1) <= max];
  return (
    <view role="group" aria-label={label} data-testid={testId} {...dataOf(props)} className={cx(r.root.base, className)}>
      <view className={r.header.base}>
        <text role="status" className={r.title.base}>{title}</text>
        <IconButton
          label={liuliText('@liuli.calendar.previousMonth')}
          icon="chevron-start"
          size="sm"
          isDisabled={!hasPrevious}
          onClick={() => go(-1)}
          data-testid={testId && `${testId}-previous`}
        />
        <IconButton
          label={liuliText('@liuli.calendar.nextMonth')}
          icon="chevron-end"
          size="sm"
          isDisabled={!hasNext}
          onClick={() => go(1)}
          data-testid={testId && `${testId}-next`}
        />
      </view>
      <view role="grid" aria-label={title} onPage={direction => { if (direction === 'next' ? hasNext : hasPrevious) go(direction === 'next' ? 1 : -1); }} className={r.grid.base}>
        <view aria-hidden className={r.week.base}>
          {ordered.map((name, index) => <text key={String(index)} className={r.weekday.base}>{name}</text>)}
        </view>
        {weeksOf(year, month, firstDay).map((week, row) => (
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
                    aria-label={liuliText('@liuli.calendar.day', {
                      today: date === now ? 'yes' : 'no', weekdayName: weekdayName(weekdayOf(year, month, day)), day, monthName: monthName(month), year: String(year),
                    })}
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
