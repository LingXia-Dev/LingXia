import assert from 'node:assert/strict';
import { formatter } from '../dist/esm/format.js';

// Each case asserts the binding (the engine's own Intl with the settings spelled out), then facts
// that hold on any engine's data. Spaces before AM/PM differ between ICU versions.
const plain = text => text.replace(/[\u00a0\u202f]/g, ' ');
const at = iso => Date.parse(iso);
const instant = at('2026-11-24T02:05:30Z');

const SETTINGS = [
  { language: 'en-US', timeZone: 'America/New_York', hourCycle: 'h12' },
  { language: 'en-US', timeZone: 'America/New_York', hourCycle: 'h23' },
  { language: 'he', timeZone: 'Asia/Jerusalem', hourCycle: 'h23' },
  { language: 'ja', timeZone: 'Asia/Tokyo', hourCycle: 'h12' },
  { language: 'ar-EG', timeZone: 'Africa/Cairo', hourCycle: 'h12' },
  { language: 'de', timeZone: 'Europe/Berlin', hourCycle: 'h23' },
];

// The shapes' options, written out again so a change to the binding's table shows here.
const SHAPE_OPTIONS = {
  short: { dateStyle: 'short' },
  medium: { dateStyle: 'medium' },
  long: { dateStyle: 'long' },
  full: { dateStyle: 'full' },
  time: { hour: 'numeric', minute: '2-digit', clock: true },
  timeSeconds: { hour: 'numeric', minute: '2-digit', second: '2-digit', clock: true },
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
  weekdayTime: { weekday: 'short', hour: 'numeric', minute: '2-digit', clock: true },
  monthDayTime: { month: 'short', day: 'numeric', hour: 'numeric', minute: '2-digit', clock: true },
  dateTime: { dateStyle: 'medium', timeStyle: 'short', clock: true },
};

function intlDate(settings, shape, timeZone = settings.timeZone) {
  const { clock, ...options } = SHAPE_OPTIONS[shape];
  const hours = clock ? (settings.hourCycle === 'h12' ? { hour12: true } : { hourCycle: 'h23' }) : {};
  return new Intl.DateTimeFormat(settings.language, { ...options, ...hours, timeZone });
}

// Binding: every shape under every setting.
for (const settings of SETTINGS) {
  const f = formatter(settings);
  for (const shape of Object.keys(SHAPE_OPTIONS)) {
    assert.equal(f.date(instant, shape), intlDate(settings, shape).format(instant), `${settings.language} ${settings.hourCycle} ${shape}`);
    assert.equal(f.date(new Date(instant), shape), f.date(instant, shape), `${shape} takes a Date`);
    assert.equal(f.date(instant, shape, { timeZone: 'Asia/Kolkata' }), intlDate(settings, shape, 'Asia/Kolkata').format(instant), `${shape} in a named zone`);
  }
}

const enH12 = formatter(SETTINGS[0]);
const enH23 = formatter(SETTINGS[1]);

// The hour cycle.
assert.equal(plain(enH12.date(instant, 'time')), '9:05 PM');
assert.equal(enH23.date(instant, 'time'), '21:05');
assert.equal(enH12.date(instant, 'time24'), '21:05', 'time24 ignores the setting');
assert.equal(plain(enH12.date(instant, 'timeSeconds')), '9:05:30 PM');
assert.match(plain(enH12.date(instant, 'weekdayTime')), /^Mon 9:05 PM$/);
assert.equal(enH12.date(instant, 'medium'), 'Nov 23, 2026', 'date styles carry no time, in the bound zone');
assert.ok(!/\d:\d/.test(enH12.date(instant, 'full')), 'full is date only');
assert.equal(plain(enH12.date(instant, 'dateTime')), 'Nov 23, 2026, 9:05 PM');
assert.equal(enH23.date(instant, 'dateTime'), 'Nov 23, 2026, 21:05');

// A named zone across a DST change: New York skips 02:00–03:00 on 2026-03-08 and repeats 01:00–02:00 on 2026-11-01.
assert.equal(enH23.date(at('2026-03-08T06:59:00Z'), 'time'), '01:59');
assert.equal(enH23.date(at('2026-03-08T07:00:00Z'), 'time'), '03:00');
assert.equal(enH23.date(at('2026-11-01T05:30:00Z'), 'time'), '01:30');
assert.equal(enH23.date(at('2026-11-01T06:30:00Z'), 'time'), '01:30');
// Jerusalem falls back on 2026-10-25, a day after the instant's evening at home.
const home = { timeZone: 'Asia/Jerusalem' };
assert.equal(plain(enH12.date(at('2026-10-24T21:30:00Z'), 'monthDayTime', home)), 'Oct 25, 12:30 AM');
assert.equal(plain(enH12.date(at('2026-10-25T21:30:00Z'), 'monthDayTime', home)), 'Oct 25, 11:30 PM');
// The device's Monday is still Sunday evening at home.
assert.equal(plain(enH12.date(at('2026-11-22T16:30:00Z'), 'weekdayTime', home)), 'Sun 6:30 PM');

// A wall time shows as given, in any zone.
for (const timeZone of ['America/New_York', 'Asia/Tokyo', 'UTC']) {
  const f = formatter({ language: 'en-US', timeZone, hourCycle: 'h12' });
  assert.equal(plain(f.date({ hour: 21, minute: 0 }, 'time')), '9:00 PM');
  assert.equal(plain(f.date({ hour: 21 }, 'time', { timeZone: 'Asia/Jerusalem' })), '9:00 PM', 'a zone option leaves a wall time alone');
  assert.equal(plain(f.date({ year: 2026, month: 3, day: 8, hour: 2, minute: 30 }, 'monthDayTime')), 'Mar 8, 2:30 AM', 'even a time New York skips');
}
assert.equal(enH23.date({ hour: 7, minute: 5, second: 9 }, 'timeSeconds'), '07:05:09');

// Numbers.
const NUMBER_CASES = [
  [1234.5678, undefined],
  [0.256, { style: 'percent' }],
  [-1234567.891, { maximumFractionDigits: 2, signDisplay: 'exceptZero' }],
  [5, { minimumFractionDigits: 2, signDisplay: 'always' }],
  [12345, { useGrouping: false }],
];
for (const settings of SETTINGS) {
  const f = formatter(settings);
  for (const [value, options] of NUMBER_CASES) {
    assert.equal(f.number(value, options), new Intl.NumberFormat(settings.language, options).format(value), `${settings.language} number ${value}`);
  }
}
assert.equal(enH12.number(1234.5), '1,234.5');
assert.equal(enH12.number(0.256, { style: 'percent' }), '26%');
assert.equal(enH12.in('de').number(1234.5), '1.234,5');
assert.match(formatter(SETTINGS[4]).number(123), /^[\u0660-\u0669]+$/, 'ar-EG writes Arabic-Indic digits');

// Lists.
for (const settings of SETTINGS) {
  const f = formatter(settings);
  for (const options of [undefined, { type: 'disjunction' }, { type: 'unit', style: 'narrow' }]) {
    assert.equal(f.list(['a', 'b', 'c'], options), new Intl.ListFormat(settings.language, options).format(['a', 'b', 'c']));
  }
}
assert.equal(enH12.list(['a', 'b', 'c']), 'a, b, and c');
assert.equal(enH12.in('es').list(['padre', 'hijo']), 'padre e hijo', 'Spanish y becomes e before an i sound');
assert.equal(enH12.in('es').list(['siete', 'ocho'], { type: 'disjunction' }), 'siete u ocho', 'Spanish o becomes u before an o sound');
assert.match(formatter(SETTINGS[2]).list(['א', 'ב']), /ו/, 'Hebrew joins with vav');

// Relative time.
for (const settings of SETTINGS) {
  const f = formatter(settings);
  for (const [value, unit, options] of [[3, 'day'], [-2, 'hour', { style: 'short' }], [-1, 'day', { numeric: 'auto' }], [1, 'quarter']]) {
    assert.equal(f.relativeTime(value, unit, options), new Intl.RelativeTimeFormat(settings.language, options).format(value, unit));
  }
}
assert.equal(enH12.relativeTime(-1, 'day', { numeric: 'auto' }), 'yesterday');
assert.equal(enH12.relativeTime(-0, 'second'), '0 seconds ago', '-0 is the past');

// Durations: one unit is a unit number; parts are unit numbers joined by a unit list.
for (const settings of SETTINGS) {
  const f = formatter(settings);
  const unit = (value, name, style = 'short') => new Intl.NumberFormat(settings.language, { style: 'unit', unit: name, unitDisplay: style }).format(value);
  assert.equal(f.duration(45, 'minute'), unit(45, 'minute'));
  assert.equal(f.duration(2, 'hour', { style: 'long' }), unit(2, 'hour', 'long'));
  assert.equal(f.duration({ hours: 2, minutes: 20 }), new Intl.ListFormat(settings.language, { type: 'unit', style: 'short' }).format([unit(2, 'hour'), unit(20, 'minute')]));
}
assert.equal(enH12.duration(45, 'minute'), '45 min');
assert.equal(enH12.duration({ hours: 2, minutes: 20 }), '2 hr, 20 min');
assert.equal(enH12.duration({ minutes: 20, days: 1 }), '1 day, 20 min', 'largest first');
assert.equal(enH12.duration({ hours: 0, minutes: 5 }), '5 min', 'zero parts dropped');
assert.equal(enH12.duration({ hours: 0, minutes: 0 }), '0 min', 'all zero in the smallest part given');
assert.equal(enH12.duration({ hours: 1, seconds: 30 }, { style: 'long' }), '1 hour, 30 seconds');

// Sizes and rates: decimal units, the unit chosen after rounding.
for (const settings of SETTINGS) {
  const f = formatter(settings);
  const unit = (value, name, digits = 1) => new Intl.NumberFormat(settings.language, { style: 'unit', unit: name, unitDisplay: 'short', maximumFractionDigits: digits }).format(value);
  assert.equal(f.bytes(1500), unit(1.5, 'kilobyte'));
  assert.equal(f.bytes(2_345_678, { maximumFractionDigits: 2 }), unit(2.35, 'megabyte', 2));
  assert.equal(f.bitRate(487_000_000), unit(487, 'megabit-per-second'));
}
assert.equal(enH12.bytes(512), '512 byte');
assert.equal(enH12.bytes(1500), '1.5 kB');
assert.equal(enH12.bytes(999_999), '1 MB', '999.999 kB rounds to 1000 kB, so the next unit');
assert.equal(enH12.bytes(3e18), '3,000 PB', 'the largest unit grows past 1000');
assert.equal(enH12.bitRate(487_000_000), '487 Mb/s');
assert.equal(enH12.bitRate(1_500), '1.5 kb/s');

// Bad input: '' and one warning per call, never a throw.
const warnings = [];
const watched = formatter({ ...SETTINGS[0], warn: message => warnings.push(message) });
const BAD = [
  () => watched.date(instant, 'time', { timeZone: 'Mars/Base' }),
  () => formatter({ ...SETTINGS[0], timeZone: 'Mars/Base', warn: message => warnings.push(message) }).date(instant, 'time'),
  () => watched.date(new Date(NaN), 'medium'),
  () => watched.date('2026-11-24', 'medium'),
  () => watched.date(instant, 'yesterday'),
  () => watched.date({ hour: 21 }, 'medium'),
  () => watched.date({ year: 2026, month: 1.5, day: 1 }, 'medium'),
  () => watched.number(NaN),
  () => watched.number('12'),
  () => watched.number(1, { minimumFractionDigits: 3, maximumFractionDigits: 1 }),
  () => watched.list('a, b'),
  () => watched.list(['a', 2]),
  () => watched.relativeTime(1, 'fortnight'),
  () => watched.relativeTime(Infinity, 'day'),
  () => watched.duration(-5, 'minute'),
  () => watched.duration(5, 'week'),
  () => watched.duration({}),
  () => watched.duration({ hours: -1 }),
  () => watched.bytes(NaN),
  () => watched.bitRate(undefined),
  () => watched.in('not a tag!').number(1),
];
for (const [index, call] of BAD.entries()) {
  const before = warnings.length;
  assert.equal(call(), '', `bad input ${index} gives ''`);
  assert.equal(warnings.length, before + 1, `bad input ${index} warns once`);
}
assert.match(warnings[0], /^format\.date: .*Mars\/Base/);
assert.equal(formatter(SETTINGS[0]).date(instant, 'time', { timeZone: 'Mars/Base' }), '', 'no warn given: still no throw');

// One formatter is built per language, shape and zone, then reused.
const RealDateTimeFormat = Intl.DateTimeFormat;
let built = 0;
Intl.DateTimeFormat = function (...args) { built++; return new RealDateTimeFormat(...args); };
try {
  const f = formatter({ language: 'en-GB', timeZone: 'Europe/London', hourCycle: 'h23' });
  for (let i = 0; i < 100; i++) f.date(instant + i * 60_000, 'monthDayTime');
  assert.equal(built, 1);
} finally {
  Intl.DateTimeFormat = RealDateTimeFormat;
}

console.log('format runtime: ok');
