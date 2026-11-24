import { formatter, type DateShape } from '../src/format.js';

const format = formatter({ language: 'en-US', timeZone: 'UTC', hourCycle: 'h12' });

const shown: string[] = [
  format.date(Date.now(), 'monthDayTime', { timeZone: 'Asia/Tokyo' }),
  format.date(new Date(), 'medium'),
  format.date({ year: 2026, month: 11, day: 24, hour: 9 }, 'weekdayMonthDay'),
  format.date({ hour: 21, minute: 0 }, 'time'),
  format.duration(45, 'minute'),
  format.duration({ hours: 2, minutes: 20 }, { style: 'long' }),
  format.list(['a', 'b'] as const, { type: 'disjunction' }),
  format.in('he').relativeTime(-1, 'day', { numeric: 'auto' }),
];
const shape: DateShape = 'numericDate';

// @ts-expect-error a shape is a closed name
format.date(Date.now(), 'yMMMd');
// @ts-expect-error a time of day needs a time-only shape
format.date({ hour: 21 }, 'medium');
// @ts-expect-error a time of day shows as given, so it takes no zone
format.date({ hour: 21 }, 'time', { timeZone: 'UTC' });
// @ts-expect-error the hour cycle is 'h12' or 'h23'
formatter({ language: 'en', timeZone: 'UTC', hourCycle: 'h24' });
// @ts-expect-error a duration unit is day to second
format.duration(1, 'week');

void shown;
void shape;
