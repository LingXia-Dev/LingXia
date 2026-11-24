import assert from 'node:assert/strict';
import { dateParts, fromDateParts, isTimeZone } from '../dist/esm/page.js';

const ny = { timeZone: 'America/New_York' };
const at = (iso) => Date.parse(iso);

// A wall time in a zone, whatever the machine's zone.
const p = dateParts(at('2026-11-22T02:30:00Z'), ny);
assert.deepEqual({ ...p }, {
  year: 2026, month: 11, day: 21, hour: 21, minute: 30, second: 0, dayOfWeek: 6, minuteOfDay: 1290,
  dayNumber: Math.floor(Date.UTC(2026, 10, 21) / 86_400_000),
});
assert.equal(dateParts(new Date(at('2026-01-01T00:00:00Z')), { timeZone: 'UTC' }).hour, 0);
// ISO numbering: 2026-11-23 is a Monday, 2026-11-22 a Sunday.
assert.equal(dateParts(at('2026-11-23T12:00:00Z'), { timeZone: 'UTC' }).dayOfWeek, 1);
assert.equal(dateParts(at('2026-11-22T12:00:00Z'), { timeZone: 'UTC' }).dayOfWeek, 7);

// Rolling over: start of today, a week on, 21:30 today.
assert.equal(fromDateParts({ ...p, hour: 0, minute: 0 }, ny), at('2026-11-21T05:00:00Z'));
assert.equal(fromDateParts({ ...p, day: p.day + 7, hour: 0, minute: 0 }, ny), at('2026-11-28T05:00:00Z'));
assert.equal(fromDateParts({ year: 2026, month: 11, day: 21, minute: 1290 }, ny), at('2026-11-22T02:30:00Z'));
assert.equal(dateParts(at('2026-11-22T02:30:00Z'), ny).dayNumber - dateParts(at('2026-11-01T12:00:00Z'), ny).dayNumber, 20);

// DST: a skipped time moves forward by the gap; a repeated time takes the earlier instant.
assert.equal(fromDateParts({ year: 2026, month: 3, day: 8, hour: 2, minute: 30 }, ny), at('2026-03-08T07:30:00Z'));
assert.equal(fromDateParts({ year: 2026, month: 11, day: 1, hour: 1, minute: 30 }, ny), at('2026-11-01T05:30:00Z'));
const jerusalem = { timeZone: 'Asia/Jerusalem' };
assert.equal(fromDateParts({ year: 2026, month: 10, day: 25, hour: 1, minute: 30 }, jerusalem), at('2026-10-24T22:30:00Z'));
// Round trip across the year.
for (let day = 0; day < 366; day += 5) {
  const instant = Date.UTC(2026, 0, 1, 12) + day * 86_400_000;
  assert.equal(fromDateParts(dateParts(instant, jerusalem), jerusalem), instant);
}

// A zone that does not exist throws E_INVALID_ARG naming it; isTimeZone answers without throwing.
assert.throws(() => dateParts(0, { timeZone: 'Mars/Olympus' }), e => e.code === 'E_INVALID_ARG' && e.data.got === 'Mars/Olympus');
assert.throws(() => fromDateParts({ year: 2026, month: 1, day: 1.5 }, ny), e => e.code === 'E_INVALID_ARG' && e.data.argument === 'day');
assert.throws(() => dateParts(Number.NaN, ny), e => e.code === 'E_INVALID_ARG' && e.data.argument === 'at');
assert.equal(isTimeZone('Europe/Berlin'), true);
assert.equal(isTimeZone('UTC'), true);
assert.equal(isTimeZone('Mars/Olympus'), false);
assert.equal(isTimeZone(undefined), false);
// No zone-aware Intl: unsupported, never "no such zone".
const intl = globalThis.Intl;
delete globalThis.Intl;
try {
  assert.throws(() => dateParts(0, { timeZone: 'Asia/Tokyo' }), e => e.code === 'E_NOT_SUPPORTED');
  assert.throws(() => isTimeZone('Asia/Tokyo'), e => e.code === 'E_NOT_SUPPORTED');
} finally {
  globalThis.Intl = intl;
}
assert.equal(dateParts(0, { timeZone: 'Asia/Tokyo' }).hour, 9);
console.log('time: ok');
