import { cx, useView } from '@lingxia/view';
import { timestampRecipe as r } from './Timestamp.recipe';

/** A moment in the user's own time: `2026-11-14T09:42`. The app converts from its clock and zone. */
export type LocalDateTime = string;

/** The words a timestamp is written in. Liuli ships `timestampEn` and `timestampZh`; spread one to change a phrase. */
export interface TimestampLocale {
  justNow: string;
  minutesAgo: (minutes: number) => string;
  hoursAgo: (hours: number) => string;
  yesterday: string;
  /** `month` from 1. `hasYear` is false for a date in the current year. */
  date: (year: number, month: number, day: number, hasYear: boolean) => string;
  /** `hour` from 0 to 23; `isTwelveHour` follows the user's clock. */
  time: (hour: number, minute: number, isTwelveHour: boolean) => string;
  /** A date and a time together: "14 November, 09:42". */
  dateTime: (date: string, time: string) => string;
}

export type TimestampFormat = 'relative' | 'date' | 'time' | 'date-time';

export interface TimestampProps {
  /** The moment, in the user's own time. */
  value: LocalDateTime;
  /** The present, in the same form. The app passes it and refreshes it; Liuli has no clock. */
  now: LocalDateTime;
  locale: TimestampLocale;
  /** `relative` says how long ago for today, "yesterday" with the time for yesterday, and the date before that. @default 'relative' */
  format?: TimestampFormat;
  className?: string;
  'data-testid'?: string;
}

type Parts = { year: number; month: number; day: number; hour: number; minute: number };

function parse(text: LocalDateTime): Parts {
  const match = /^(\d{4})-(\d{2})-(\d{2})T(\d{2}):(\d{2})/.exec(text);
  if (!match) throw Error(`Timestamp: expected YYYY-MM-DDTHH:mm, got ${text}`);
  const [, year, month, day, hour, minute] = match.map(Number);
  return { year, month, day, hour, minute };
}

/** Days since 1970-01-01 in the proleptic Gregorian calendar. */
function dayNumber({ year, month, day }: Parts): number {
  const y = month <= 2 ? year - 1 : year;
  const era = Math.floor(y / 400);
  const yearOfEra = y - era * 400;
  const dayOfYear = Math.floor((153 * (month + (month > 2 ? -3 : 9)) + 2) / 5) + day - 1;
  return era * 146097 + yearOfEra * 365 + Math.floor(yearOfEra / 4) - Math.floor(yearOfEra / 100) + dayOfYear - 719468;
}

/** The words for `value` seen from `now`. */
export function formatTimestamp(value: LocalDateTime, now: LocalDateTime, locale: TimestampLocale, format: TimestampFormat, isTwelveHour: boolean): string {
  const [then, present] = [parse(value), parse(now)];
  const time = locale.time(then.hour, then.minute, isTwelveHour);
  const date = locale.date(then.year, then.month, then.day, then.year !== present.year);
  if (format === 'time') return time;
  if (format === 'date') return date;
  if (format === 'date-time') return locale.dateTime(date, time);
  const days = dayNumber(present) - dayNumber(then);
  const minutes = days * 1440 + (present.hour - then.hour) * 60 + (present.minute - then.minute);
  if (minutes < 0) return locale.dateTime(date, time);
  if (minutes < 1) return locale.justNow;
  if (minutes < 60) return locale.minutesAgo(minutes);
  if (days === 0) return locale.hoursAgo(Math.floor(minutes / 60));
  if (days === 1) return locale.dateTime(locale.yesterday, time);
  return date;
}

/** When something happened, in the words people use for it. Read in full: the date and the time. */
export function Timestamp({ value, now, locale, format = 'relative', className, 'data-testid': testId }: TimestampProps) {
  const isTwelveHour = useView().env.hourCycle === 'h12';
  const shown = formatTimestamp(value, now, locale, format, isTwelveHour);
  const full = formatTimestamp(value, now, locale, 'date-time', isTwelveHour);
  return (
    <text data-testid={testId} aria-label={shown === full ? undefined : full} className={cx(r.root.base, className)}>
      {shown}
    </text>
  );
}
