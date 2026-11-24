import type { WallTime } from './time.js';

/** What a `Format` is bound to; the runtime supplies it from the app's settings. */
export interface FormatSettings {
  /** The app's language, a BCP-47 tag such as `'he'` or `'zh-CN'`. */
  readonly language: string;
  /** The zone times show in, an IANA name such as `'Europe/Berlin'`, or `'UTC'`. */
  readonly timeZone: string;
  /** 12- or 24-hour clock for the shapes that follow the clock setting. */
  readonly hourCycle: 'h12' | 'h23';
  /** Told once per call that got bad input; the runtime passes it in development builds. */
  readonly warn?: (message: string) => void;
}

/** The named date shapes; `date` takes a name. Examples are en-US, 12-hour. */
export interface DateShapes {
  /** Date only, the language's short style: "11/24/26". For dense tables. */
  readonly short: true;
  /** Date only, the language's medium style: "Nov 24, 2026". The usual full date. */
  readonly medium: true;
  /** Date only, the language's long style: "November 24, 2026". */
  readonly long: true;
  /** Date only, the language's full style: "Tuesday, November 24, 2026". */
  readonly full: true;
  /** Hour and minute on the user's clock: "9:05 PM". */
  readonly time: true;
  /** Hour, minute and second on the user's clock: "9:05:30 PM". */
  readonly timeSeconds: true;
  /** Hour and minute on a 24-hour clock, whatever the user's setting: "21:05". */
  readonly time24: true;
  /** Month and day: "Nov 24". For dates in the current year. */
  readonly monthDay: true;
  /** Month and day, month spelled out: "November 24". */
  readonly monthDayLong: true;
  /** Month and day in digits: "11/24". */
  readonly numericMonthDay: true;
  /** Weekday, month and day: "Tue, Nov 24". */
  readonly weekdayMonthDay: true;
  /** Weekday, month and day, spelled out: "Tuesday, November 24". */
  readonly weekdayMonthDayLong: true;
  /** Month, day and year: "Nov 24, 2026". */
  readonly monthDayYear: true;
  /** The date in digits: "11/24/2026". */
  readonly numericDate: true;
  /** Month and year: "November 2026". For calendar headers. */
  readonly monthYear: true;
  /** The weekday: "Tuesday". */
  readonly weekday: true;
  /** Date and time: weekday and the time on the user's clock: "Tue 9:05 PM". For times within the week. */
  readonly weekdayTime: true;
  /** Date and time: month, day and the time on the user's clock: "Nov 24, 9:05 PM". */
  readonly monthDayTime: true;
}

/** A date shape's name. */
export type DateShape = keyof DateShapes;

/** The shapes that show only a time, so they accept a `TimeOfDay`. */
export type TimeShape = 'time' | 'timeSeconds' | 'time24';

/** A time of day with no date, such as a schedule's 21:00. */
export interface TimeOfDay {
  /** 0–23. */
  readonly hour: number;
  /** Defaults to 0. */
  readonly minute?: number;
  /** Defaults to 0. */
  readonly second?: number;
}

/** Options for `date`. */
export interface DateOptions {
  /** Show the instant in this IANA zone instead of the bound one; ignored for a wall time. */
  readonly timeZone?: string;
}

/** Options for `number`. */
export interface NumberOptions {
  /** `'percent'` multiplies by 100 and adds the language's percent sign. @default 'decimal' */
  readonly style?: 'decimal' | 'percent';
  /** At least this many fraction digits. @default 0 */
  readonly minimumFractionDigits?: number;
  /** At most this many fraction digits, rounding half away from zero. @default 3 (0 for percent) */
  readonly maximumFractionDigits?: number;
  /** `'always'` and `'exceptZero'` show a plus sign. @default 'auto' */
  readonly signDisplay?: 'auto' | 'always' | 'exceptZero';
  /** `false` drops the thousands separators. @default the language's rule */
  readonly useGrouping?: boolean;
}

/** Options for `list`. */
export interface ListOptions {
  /** `'conjunction'` "a, b, and c"; `'disjunction'` "a, b, or c"; `'unit'` "a, b, c" for measures. @default 'conjunction' */
  readonly type?: 'conjunction' | 'disjunction' | 'unit';
  /** How much the joiners are shortened. @default 'long' */
  readonly style?: 'long' | 'short' | 'narrow';
}

/** A unit for `relativeTime`. */
export type RelativeTimeUnit = 'second' | 'minute' | 'hour' | 'day' | 'week' | 'month' | 'quarter' | 'year';

/** Options for `relativeTime`. */
export interface RelativeTimeOptions {
  /** `'auto'` allows words such as "yesterday"; `'always'` keeps the number, "1 day ago". @default 'always' */
  readonly numeric?: 'always' | 'auto';
  /** How much the units are shortened. @default 'long' */
  readonly style?: 'long' | 'short' | 'narrow';
}

/** A unit for `duration`. */
export type DurationUnit = 'day' | 'hour' | 'minute' | 'second';

/** A duration in parts, each zero or more. */
export interface DurationParts {
  readonly days?: number;
  readonly hours?: number;
  readonly minutes?: number;
  readonly seconds?: number;
}

/** Options for `duration`. */
export interface DurationOptions {
  /** `'long'` "2 hours", `'short'` "2 hr", `'narrow'` "2h". @default 'short' */
  readonly style?: 'long' | 'short' | 'narrow';
}

/** Options for `bytes` and `bitRate`. */
export interface UnitScaleOptions {
  /** At most this many fraction digits. @default 1 */
  readonly maximumFractionDigits?: number;
}

/**
 * Numbers, dates, lists and durations as text in the app's language, zone and hour cycle. Use it
 * instead of a bare `Intl` call for text a user reads: an engine's default locale is the phone's,
 * not the app's. Every member is synchronous and pure, and returns `''` for bad input instead of
 * throwing.
 */
export interface Format {
  /** A number: "1,234.5", "25%". For counts, amounts and ratios outside a message. */
  number(value: number, options?: NumberOptions): string;
  /**
   * An instant in the bound zone (or `options.timeZone`), in a named shape. A `WallTime` or a
   * `TimeOfDay` is shown as given, in no zone: use it for a schedule's local time. A `TimeOfDay`
   * needs a time-only shape.
   *
   * ```ts
   * format.date(offAt, 'weekdayTime', { timeZone: home });  // "Sun 6:30 PM" at home
   * format.date({ hour: 21, minute: 0 }, 'time');           // "9:00 PM" in any zone
   * ```
   */
  date(at: Date | number | WallTime, shape: DateShape, options?: DateOptions): string;
  date(at: TimeOfDay, shape: TimeShape): string;
  /** Items joined as the language joins them: "a, b, and c", Spanish "padre e hijo". */
  list(items: readonly string[], options?: ListOptions): string;
  /** Time from now: "in 3 days", "2 hours ago"; a negative value is the past. For feeds and reminders. */
  relativeTime(value: number, unit: RelativeTimeUnit, options?: RelativeTimeOptions): string;
  /** How long something lasts, in one unit: "45 min". */
  duration(value: number, unit: DurationUnit, options?: DurationOptions): string;
  /**
   * How long something lasts, in parts: "2 hr, 20 min". Zero parts are dropped, the largest comes
   * first, and all zero shows "0" in the smallest part given.
   */
  duration(parts: DurationParts, options?: DurationOptions): string;
  /** A size in decimal units (1 kB = 1000 bytes): "1.5 MB". For files and storage. */
  bytes(value: number, options?: UnitScaleOptions): string;
  /** A data rate in decimal units: "487 Mb/s". `bitsPerSecond` counts bits, not bytes. */
  bitRate(bitsPerSecond: number, options?: UnitScaleOptions): string;
  /** The same members in another language, for text that is not in the app's language, such as a sample in a language picker. */
  in(language: string): Format;
}

type Options = Record<string, string | number | boolean>;

const SHAPES: Record<DateShape, Options> = {
  short: { dateStyle: 'short' },
  medium: { dateStyle: 'medium' },
  long: { dateStyle: 'long' },
  full: { dateStyle: 'full' },
  time: { hour: 'numeric', minute: '2-digit' },
  timeSeconds: { hour: 'numeric', minute: '2-digit', second: '2-digit' },
  time24: { hour: '2-digit', minute: '2-digit', hourCycle: 'h23' },
  monthDay: { month: 'short', day: 'numeric' },
  monthDayLong: { month: 'long', day: 'numeric' },
  numericMonthDay: { month: 'numeric', day: 'numeric' },
  weekdayMonthDay: { weekday: 'short', month: 'short', day: 'numeric' },
  weekdayMonthDayLong: { weekday: 'long', month: 'long', day: 'numeric' },
  monthDayYear: { year: 'numeric', month: 'short', day: 'numeric' },
  numericDate: { year: 'numeric', month: 'numeric', day: 'numeric' },
  monthYear: { year: 'numeric', month: 'long' },
  weekday: { weekday: 'long' },
  weekdayTime: { weekday: 'short', hour: 'numeric', minute: '2-digit' },
  monthDayTime: { month: 'short', day: 'numeric', hour: 'numeric', minute: '2-digit' },
};
const TIME_SHAPES: ReadonlySet<string> = new Set<TimeShape>(['time', 'timeSeconds', 'time24']);
const DURATION_PARTS = [['days', 'day'], ['hours', 'hour'], ['minutes', 'minute'], ['seconds', 'second']] as const;
const BYTES = ['byte', 'kilobyte', 'megabyte', 'gigabyte', 'terabyte', 'petabyte'];
const BIT_RATES = ['bit-per-second', 'kilobit-per-second', 'megabit-per-second', 'gigabit-per-second', 'terabit-per-second'];

// `Intl.ListFormat` is ES2021; typed here so the package's declarations need no newer lib.
type ListFormatter = { format(items: readonly string[]): string };
type IntlWithList = typeof Intl & { ListFormat: new (language: string, options: Options) => ListFormatter };

const cache = new Map<string, unknown>();

// Building a formatter costs far more than using one; an app uses few languages, zones and shapes.
function cached<T>(kind: string, language: string, options: Options, make: () => T): T {
  const key = `${kind}\u0000${language}\u0000${JSON.stringify(options)}`;
  let found = cache.get(key) as T | undefined;
  if (found === undefined) {
    if (cache.size >= 256) cache.clear();
    found = make();
    cache.set(key, found);
  }
  return found;
}

const numberFormat = (language: string, options: Options) =>
  cached('n', language, options, () => new Intl.NumberFormat(language, options));
const dateFormat = (language: string, options: Options) =>
  cached('d', language, options, () => new Intl.DateTimeFormat(language, options));
const listFormat = (language: string, options: Options) =>
  cached('l', language, options, () => new (Intl as IntlWithList).ListFormat(language, options));
const relativeFormat = (language: string, options: Options) =>
  cached('r', language, options, () => new Intl.RelativeTimeFormat(language, options));

class BadInput extends Error {}

function defined(options: object | undefined): Options {
  const out: Options = {};
  for (const [key, value] of Object.entries(options ?? {})) if (value !== undefined) out[key] = value;
  return out;
}

function finite(value: unknown, what: string): number {
  if (typeof value !== 'number' || !Number.isFinite(value)) throw new BadInput(`${what}: expected a finite number, got ${String(value)}`);
  return value;
}

function integer(value: unknown, what: string): number {
  if (!Number.isInteger(value)) throw new BadInput(`${what}: expected an integer, got ${String(value)}`);
  return value as number;
}

// A wall time as a UTC instant, so formatting it in UTC shows the fields as given.
function wallInstant(at: Partial<WallTime>, shape: DateShape): number {
  const day = new Date(0);
  if (at.year === undefined) {
    if (!TIME_SHAPES.has(shape)) throw new BadInput(`a time of day needs a time shape, got '${shape}'`);
  } else {
    day.setUTCFullYear(integer(at.year, 'year'), integer(at.month, 'month') - 1, integer(at.day, 'day'));
  }
  day.setUTCHours(integer(at.hour ?? 0, 'hour'), integer(at.minute ?? 0, 'minute'), integer(at.second ?? 0, 'second'));
  return day.getTime();
}

// Picks the largest unit that keeps the rounded value under 1000.
function scaled(language: string, value: number, units: readonly string[], digits: number): string {
  let step = 0;
  const factor = 10 ** digits;
  while (step < units.length - 1 && Math.round((Math.abs(value) / 1000 ** step) * factor) / factor >= 1000) step++;
  const options = { style: 'unit', unit: units[step], unitDisplay: 'short', maximumFractionDigits: digits };
  return numberFormat(language, options).format(value / 1000 ** step);
}

/**
 * A `Format` bound to `settings`. Logic and the View export one already bound to the app; call
 * this only to bind other settings, such as in a test.
 */
export function formatter(settings: FormatSettings): Format {
  const { language, hourCycle } = settings;

  function guard<A extends unknown[]>(member: string, run: (...args: A) => string): (...args: A) => string {
    return (...args) => {
      try {
        return run(...args);
      } catch (error) {
        settings.warn?.(`format.${member}: ${error instanceof Error ? error.message : String(error)}`);
        return '';
      }
    };
  }

  function date(at: Date | number | WallTime | TimeOfDay, shape: DateShape, options?: DateOptions): string {
    const spec = Object.prototype.hasOwnProperty.call(SHAPES, shape) ? SHAPES[shape] : undefined;
    if (!spec) throw new BadInput(`unknown shape '${String(shape)}'`);
    let ms: number;
    let timeZone = options?.timeZone ?? settings.timeZone;
    if (at instanceof Date) ms = at.getTime();
    else if (typeof at === 'object' && at !== null) {
      ms = wallInstant(at as Partial<WallTime>, shape);
      timeZone = 'UTC';
    } else ms = at;
    finite(ms, 'at');
    // `hour12` rather than `hourCycle: 'h12'` lets a language that counts 0–11 (ja) keep its clock.
    const clock: Options = 'hour' in spec && !('hourCycle' in spec) ? (hourCycle === 'h12' ? { hour12: true } : { hourCycle: 'h23' }) : {};
    return dateFormat(language, { ...spec, ...clock, timeZone }).format(ms);
  }

  function duration(value: number | DurationParts, unitOrOptions?: DurationUnit | DurationOptions, maybe?: DurationOptions): string {
    // No engine-wide `Intl.DurationFormat` (rong's QuickJS and older JavaScriptCore lack it): unit numbers joined by a unit list.
    const single = typeof value === 'number';
    const style = (single ? maybe : (unitOrOptions as DurationOptions | undefined))?.style ?? 'short';
    const unitText = (amount: number, unit: string) =>
      numberFormat(language, { style: 'unit', unit, unitDisplay: style }).format(amount);
    if (single) {
      const unit = unitOrOptions as DurationUnit;
      if (!DURATION_PARTS.some(([, name]) => name === unit)) throw new BadInput(`unknown unit '${String(unit)}'`);
      if (finite(value, 'value') < 0) throw new BadInput(`value: expected zero or more, got ${value}`);
      return unitText(value, unit);
    }
    if (typeof value !== 'object' || value === null) throw new BadInput(`expected a number or parts, got ${String(value)}`);
    const given = DURATION_PARTS.filter(([key]) => value[key] !== undefined);
    if (given.length === 0) throw new BadInput('parts: expected at least one of days, hours, minutes, seconds');
    const shown: string[] = [];
    for (const [key, unit] of given) {
      const amount = finite(value[key], key);
      if (amount < 0) throw new BadInput(`${key}: expected zero or more, got ${amount}`);
      if (amount > 0) shown.push(unitText(amount, unit));
    }
    if (shown.length === 0) return unitText(0, given[given.length - 1][1]);
    return listFormat(language, { type: 'unit', style }).format(shown);
  }

  return {
    number: guard('number', (value: number, options?: NumberOptions) =>
      numberFormat(language, defined(options)).format(finite(value, 'value'))),
    date: guard('date', date) as Format['date'],
    list: guard('list', (items: readonly string[], options?: ListOptions) => {
      if (!Array.isArray(items) || items.some(item => typeof item !== 'string')) throw new BadInput('items: expected an array of strings');
      return listFormat(language, defined(options)).format(items);
    }),
    relativeTime: guard('relativeTime', (value: number, unit: RelativeTimeUnit, options?: RelativeTimeOptions) =>
      relativeFormat(language, defined(options)).format(finite(value, 'value'), unit)),
    duration: guard('duration', duration) as Format['duration'],
    bytes: guard('bytes', (value: number, options?: UnitScaleOptions) =>
      scaled(language, finite(value, 'value'), BYTES, options?.maximumFractionDigits ?? 1)),
    bitRate: guard('bitRate', (value: number, options?: UnitScaleOptions) =>
      scaled(language, finite(value, 'bitsPerSecond'), BIT_RATES, options?.maximumFractionDigits ?? 1)),
    in: (other: string) => formatter({ ...settings, language: other }),
  };
}
