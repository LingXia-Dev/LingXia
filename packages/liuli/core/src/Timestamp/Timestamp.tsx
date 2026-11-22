import { cx, useView } from '@lingxia/view';
import { liuliText, monthName } from '../i18n/text';
import { timestampRecipe as r } from './Timestamp.recipe';

/** A moment in the user's own time: `2026-11-14T09:42`. The app converts from its clock and zone. */
export type LocalDateTime = string;

export type TimestampFormat = 'relative' | 'date' | 'time' | 'date-time';

export interface TimestampProps {
  /** The moment, in the user's own time. */
  value: LocalDateTime;
  /** The present, in the same form. The app passes it and refreshes it; Liuli has no clock. */
  now: LocalDateTime;
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

const two = (n: number) => String(n).padStart(2, '0');

/** The words for `value` seen from `now`, in the page's language. During render only. */
export function formatTimestamp(value: LocalDateTime, now: LocalDateTime, format: TimestampFormat, isTwelveHour: boolean): string {
  const [then, present] = [parse(value), parse(now)];
  const time = isTwelveHour
    ? liuliText('@liuli.timestamp.time12', { hour: String(then.hour % 12 || 12), minute: two(then.minute), period: then.hour < 12 ? 'am' : 'pm' })
    : `${two(then.hour)}:${two(then.minute)}`;
  const day = { day: String(then.day), monthName: monthName(then.month), year: String(then.year) };
  const date = then.year !== present.year ? liuliText('@liuli.timestamp.dateWithYear', day) : liuliText('@liuli.timestamp.date', { day: day.day, monthName: day.monthName });
  const dateTime = (date: string) => liuliText('@liuli.timestamp.dateTime', { date, time });
  if (format === 'time') return time;
  if (format === 'date') return date;
  if (format === 'date-time') return dateTime(date);
  const days = dayNumber(present) - dayNumber(then);
  const minutes = days * 1440 + (present.hour - then.hour) * 60 + (present.minute - then.minute);
  if (minutes < 0) return dateTime(date);
  if (minutes < 1) return liuliText('@liuli.timestamp.justNow');
  if (minutes < 60) return liuliText('@liuli.timestamp.minutesAgo', { minutes });
  if (days === 0) return liuliText('@liuli.timestamp.hoursAgo', { hours: Math.floor(minutes / 60) });
  if (days === 1) return dateTime(liuliText('@liuli.timestamp.yesterday'));
  return date;
}

/** When something happened, in the words people use for it. Read in full: the date and the time. */
export function Timestamp({ value, now, format = 'relative', className, 'data-testid': testId }: TimestampProps) {
  const isTwelveHour = useView().env.hourCycle === 'h12';
  const shown = formatTimestamp(value, now, format, isTwelveHour);
  const full = formatTimestamp(value, now, 'date-time', isTwelveHour);
  return (
    <text data-testid={testId} aria-label={shown === full ? undefined : full} className={cx(r.root.base, className)}>
      {shown}
    </text>
  );
}
