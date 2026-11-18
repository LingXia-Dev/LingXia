import { useLocal } from '@lingxia/view';
import { unused } from '../example';
import { Calendar, calendarWeekday, type CalendarRange } from './Calendar';
import { calendarEn } from './locale/en';
import { calendarZh } from './locale/zh';

const today = '2026-11-15';

/** One day: the chosen day is filled, today is ringed. */
export function Day() {
  const day = useLocal<string | null>('day', '2026-11-19');
  return <Calendar label="Due date" locale={calendarEn} value={day.value} onChange={day.set} today={today} data-testid="due" />;
}

/** A span: the first press starts it, the second ends it. Days before today cannot be chosen. */
export function Stay() {
  const stay = useLocal<CalendarRange>('stay', { start: '2026-11-18', end: '2026-11-24' });
  return (
    <view className="gap-2">
      <Calendar label="Stay" locale={calendarEn} range={stay.value} onRangeChange={stay.set} min={today} today={today} data-testid="stay" />
      <text role="status" className="px-2 text-footnote text-muted tabular-nums">{`${stay.value.start ?? ''} – ${stay.value.end ?? ''}`}</text>
    </view>
  );
}

const closed = (date: string) => calendarWeekday(date) % 6 === 0;

/** Days the app rules out stay in the grid, dimmed: here the weekends. */
export function Weekdays() {
  const day = useLocal<string | null>('day', null);
  return <Calendar label="Appointment" locale={calendarEn} value={day.value} onChange={day.set} isDateDisabled={closed} today={today} month="2026-12" data-testid="appointment" />;
}

/** Another language: the words and the first day of the week come from the locale. */
export function Chinese() {
  const day = useLocal<string | null>('day', '2026-11-19');
  return <Calendar label="截止日期" locale={calendarZh} value={day.value} onChange={day.set} today={today} data-testid="zh" />;
}

/** The home tile. */
export function Specimen() {
  // A month at half size does not fit a tile: its head and first weeks, fading out where the tile cuts them.
  return (
    <view className="w-[130px] h-[88px] overflow-hidden mask-b-from-50%">
      <Calendar label="Calendar" locale={calendarEn} value="2026-11-05" onChange={unused} today={today} className="w-[260px] scale-50 ltr:origin-top-left rtl:origin-top-right" />
    </view>
  );
}
