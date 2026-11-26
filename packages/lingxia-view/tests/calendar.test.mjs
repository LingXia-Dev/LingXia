import { test } from 'node:test';
import assert from 'node:assert/strict';
import { addDays, addMonths, monthGrid, moveDate, weekday, weekdayOrder, parseDate, parseTime, iso, wordsFor, minuteValues, secondValues, hourValues, within, dateFormat, fieldText } from '../src/runtime/calendar.js';

test('day arithmetic crosses months, years and leap days', () => {
  assert.equal(iso(addDays({ year: 2024, month: 2, day: 28 }, 1)), '2024-02-29');
  assert.equal(iso(addDays({ year: 2023, month: 12, day: 31 }, 1)), '2024-01-01');
  assert.equal(iso(addDays({ year: 2024, month: 3, day: 1 }, -1)), '2024-02-29');
  assert.equal(iso(addMonths({ year: 2024, month: 1, day: 31 }, 1)), '2024-02-29');
  assert.equal(iso(addMonths({ year: 2024, month: 1, day: 15 }, -1)), '2023-12-15');
  assert.equal(weekday(2026, 10, 29), 4);
});

test('a month grid is six weeks from the configured first weekday', () => {
  const sunday = monthGrid({ year: 2026, month: 10 }, 7);
  assert.equal(sunday.length, 42);
  assert.equal(iso(sunday[0].date), '2026-09-27');
  assert.equal(sunday.filter(cell => cell.inMonth).length, 31);
  assert.equal(iso(monthGrid({ year: 2026, month: 10 }, 1)[0].date), '2026-09-28');
  assert.equal(iso(monthGrid({ year: 2026, month: 10 }, 6)[0].date), '2026-09-26');
  assert.deepEqual(weekdayOrder(1), [1, 2, 3, 4, 5, 6, 0]);
  assert.deepEqual(weekdayOrder(7), [0, 1, 2, 3, 4, 5, 6]);
  assert.deepEqual(weekdayOrder(6), [6, 0, 1, 2, 3, 4, 5]);
});

test('keys move by day, week and month, mirrored in RTL', () => {
  const day = { year: 2026, month: 10, day: 31 };
  assert.equal(iso(moveDate(day, 'ArrowRight')), '2026-11-01');
  assert.equal(iso(moveDate(day, 'ArrowRight', true)), '2026-10-30');
  assert.equal(iso(moveDate(day, 'ArrowUp')), '2026-10-24');
  assert.equal(iso(moveDate(day, 'PageDown')), '2026-11-30');
  assert.equal(moveDate(day, 'a'), null);
});

test('values parse strictly; bounds, steps and words follow the inputs', () => {
  assert.equal(parseDate('2026-02-30'), null);
  assert.deepEqual(parseTime('07:05:30'), { hour: 7, minute: 5, second: 30 });
  assert.deepEqual(parseTime('07:05'), { hour: 7, minute: 5 });
  assert.equal(parseTime('07:05:60'), null);
  assert.deepEqual(secondValues(15), [0, 15, 30, 45]);
  assert.equal(secondValues(60), null);
  assert.equal(within({ year: 2026, month: 1, day: 1 }, parseDate('2026-01-02'), null), false);
  assert.deepEqual(minuteValues(900), [0, 15, 30, 45]);
  assert.equal(hourValues('h12')[0], 12);
  assert.equal(wordsFor('zh-Hans-CN').done, '完成');
  assert.equal(wordsFor('sv-SE').done, 'Done');
});

test('a date field reads in the user\'s language and region, English where there is no table', () => {
  const on = { year: 2026, month: 3, day: 5 };
  const cases = {
    'en-US': 'Mar 5, 2026', en: 'Mar 5, 2026', 'en-GB': '5 Mar 2026', 'en-AU': '5 Mar 2026',
    'zh-Hans-CN': '2026年3月5日', ja: '2026/03/05', ko: '2026. 3. 5.', de: '05.03.2026',
    fr: '5 mars 2026', es: '5 mar 2026', 'pt-BR': '5 de mar. de 2026', ru: '5 мар. 2026 г.',
    ar: '5‏/3‏/2026', it: 'Mar 5, 2026', '': 'Mar 5, 2026',
  };
  for (const [language, want] of Object.entries(cases)) assert.equal(dateFormat(language)(on), want, language);
  assert.deepEqual(fieldText('date', 'not a date', { language: 'en' }).displayValue, 'not a date', 'a value that is no date shows as given');
});
