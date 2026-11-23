import { LxError } from './error.js';

/** A day of the week, 1 = Monday … 7 = Sunday, as Temporal and ISO 8601; `Date.getDay()` counts 0 = Sunday. */
export type Weekday = 1 | 2 | 3 | 4 | 5 | 6 | 7;

/** A wall time in one time zone. */
export interface DateParts {
  /** The full year, such as 2026. */
  readonly year: number;
  /** 1–12, not `Date`'s 0–11. */
  readonly month: number;
  /** The day of the month, from 1. */
  readonly day: number;
  /** 0–23. */
  readonly hour: number;
  /** 0–59. */
  readonly minute: number;
  /** 0–59. */
  readonly second: number;
  /** 1 = Monday … 7 = Sunday, as Temporal's `dayOfWeek`. */
  readonly dayOfWeek: Weekday;
  /** `hour * 60 + minute`, 0–1439. */
  readonly minuteOfDay: number;
  /** Calendar days since 1970-01-01: equal for the same day, and two days' difference is the days between, whatever DST does. */
  readonly dayNumber: number;
}

/** The wall time `fromDateParts` reads; a field out of range rolls over (`day: d + 1` is tomorrow). */
export interface WallTime {
  /** The full year. */
  readonly year: number;
  /** 1–12; 13 is January of the next year. */
  readonly month: number;
  /** The day of the month; 0 is the last day of the month before. */
  readonly day: number;
  /** Defaults to 0. */
  readonly hour?: number;
  /** Defaults to 0; 1290 is 21:30. */
  readonly minute?: number;
  /** Defaults to 0. */
  readonly second?: number;
}

/** The zone to compute in. */
export interface TimeZoneOptions {
  /** An IANA zone name such as `'Europe/Berlin'`, or `'UTC'`. Check one from a server with `isTimeZone` where it enters the app. */
  readonly timeZone: string;
}

const DAY_MS = 86_400_000;
const WEEKDAYS: Record<string, Weekday> = { Mon: 1, Tue: 2, Wed: 3, Thu: 4, Fri: 5, Sat: 6, Sun: 7 };
const formatters = new Map<string, Intl.DateTimeFormat>();

function formatter(timeZone: string): Intl.DateTimeFormat | null {
  let found = formatters.get(timeZone);
  if (found) return found;
  // An engine without zone-aware Intl must not read as "no such zone".
  if (typeof Intl === 'undefined' || typeof Intl.DateTimeFormat?.prototype?.formatToParts !== 'function') {
    throw new LxError('Intl.DateTimeFormat with time zones is not available in this engine', { code: 'E_NOT_SUPPORTED' });
  }
  try {
    found = new Intl.DateTimeFormat('en-US', {
      timeZone, hourCycle: 'h23', weekday: 'short',
      year: 'numeric', month: 'numeric', day: 'numeric', hour: 'numeric', minute: 'numeric', second: 'numeric',
    });
  } catch {
    return null;
  }
  formatters.set(timeZone, found);
  return found;
}

function invalid(argument: string, expected: string, got: unknown): LxError {
  return new LxError(`${argument}: expected ${expected}, got ${JSON.stringify(got)}`, {
    code: 'E_INVALID_ARG', data: { argument, expected, got },
  });
}

function zone(options: TimeZoneOptions): Intl.DateTimeFormat {
  const timeZone = options?.timeZone;
  const found = typeof timeZone === 'string' ? formatter(timeZone) : null;
  if (!found) throw invalid('timeZone', 'an IANA time zone name', timeZone);
  return found;
}

function read(ms: number, format: Intl.DateTimeFormat): DateParts {
  const field: Record<string, string> = {};
  for (const part of format.formatToParts(ms)) field[part.type] = part.value;
  const year = Number(field.year), month = Number(field.month), day = Number(field.day);
  // Some engines write midnight as 24 even under h23.
  const hour = Number(field.hour) % 24, minute = Number(field.minute), second = Number(field.second);
  return {
    year, month, day, hour, minute, second,
    dayOfWeek: WEEKDAYS[field.weekday],
    minuteOfDay: hour * 60 + minute,
    dayNumber: Math.floor(Date.UTC(year, month - 1, day) / DAY_MS),
  };
}

// The zone's offset from UTC at `ms`, in ms.
function offsetAt(ms: number, format: Intl.DateTimeFormat): number {
  const whole = Math.floor(ms / 1000) * 1000;
  const p = read(whole, format);
  return Date.UTC(p.year, p.month - 1, p.day, p.hour, p.minute, p.second) - whole;
}

/**
 * The wall time at `at` in `timeZone`.
 *
 * ```ts
 * const late = dateParts(Date.now(), { timeZone: home }).minuteOfDay >= bedtime;
 * ```
 * @throws {E_INVALID_ARG} `at` is not a valid time, or `timeZone` names no zone.
 * @throws {E_NOT_SUPPORTED} The engine has no zone-aware `Intl.DateTimeFormat`.
 */
export function dateParts(at: Date | number, options: TimeZoneOptions): DateParts {
  const format = zone(options);
  const ms = at instanceof Date ? at.getTime() : at;
  if (typeof ms !== 'number' || !Number.isFinite(ms)) throw invalid('at', 'a Date or epoch milliseconds', at);
  return read(ms, format);
}

/**
 * The instant a wall time in `timeZone` names, in epoch ms. Fields out of range roll over, so start
 * of day, a week on and minutes past midnight are each one call. A time a DST change skips moves
 * forward by the change; a time that occurs twice takes the earlier instant.
 *
 * ```ts
 * const p = dateParts(now, { timeZone: home });
 * fromDateParts({ ...p, hour: 0, minute: 0 }, { timeZone: home });                  // start of today
 * fromDateParts({ ...p, day: p.day + 7, hour: 0, minute: 0 }, { timeZone: home });  // a week on, at midnight
 * fromDateParts({ ...p, hour: 0, minute: 1290 }, { timeZone: home });                // 21:30 today
 * ```
 * @throws {E_INVALID_ARG} A field is not an integer, or `timeZone` names no zone.
 * @throws {E_NOT_SUPPORTED} The engine has no zone-aware `Intl.DateTimeFormat`.
 */
export function fromDateParts(parts: WallTime, options: TimeZoneOptions): number {
  const format = zone(options);
  const field = (name: keyof WallTime, fallback?: number): number => {
    const value = parts?.[name] ?? fallback;
    if (!Number.isInteger(value)) throw invalid(name, 'an integer', value);
    return value as number;
  };
  const local = Date.UTC(field('year'), field('month') - 1, field('day'), field('hour', 0), field('minute', 0), field('second', 0));
  const before = offsetAt(local - DAY_MS, format);
  const after = offsetAt(local + DAY_MS, format);
  const fits = [local - before, local - after].filter(at => offsetAt(at, format) === local - at);
  // Both fit in an overlap (the earlier wins); none in a gap, which moves forward by its length.
  return fits.length > 0 ? Math.min(...fits) : local - before;
}

/**
 * Whether `name` is a time zone `dateParts` and `fromDateParts` accept. Check a zone from a server once, where it enters the app.
 * @throws {E_NOT_SUPPORTED} The engine has no zone-aware `Intl.DateTimeFormat`.
 */
export function isTimeZone(name: string): boolean {
  return typeof name === 'string' && formatter(name) !== null;
}
