import { useLocal } from '@lingxia/view';
import { unused } from '../example';
import type { CalendarRange } from '../Calendar/Calendar';
import { calendarEn } from '../Calendar/locale/en';
import { DateRangeInput } from './DateRangeInput';

const today = '2026-11-15';
const months = ['Jan', 'Feb', 'Mar', 'Apr', 'May', 'Jun', 'Jul', 'Aug', 'Sep', 'Oct', 'Nov', 'Dec'];
const short = (date: string) => `${Number(date.slice(8))} ${months[Number(date.slice(5, 7)) - 1]}`;

/** A span of days in a form: pressing the field opens a calendar, and the second day closes it. */
export function Stay() {
  const stay = useLocal<CalendarRange>('stay', { start: '2026-11-18', end: '2026-11-24' });
  return (
    <DateRangeInput
      label="Stay"
      description="Check-in from 3 pm, check-out by 11 am."
      value={stay.value}
      onChange={stay.set}
      format={short}
      locale={calendarEn}
      today={today}
      min={today}
      closeLabel="Close"
      data-testid="stay"
    />
  );
}

/** Nothing chosen yet, and required. */
export function Empty() {
  const leave = useLocal<CalendarRange>('leave', { start: null, end: null });
  return (
    <DateRangeInput
      label="Time off"
      placeholder="First and last day"
      isRequired
      value={leave.value}
      onChange={leave.set}
      format={short}
      locale={calendarEn}
      today={today}
      data-testid="leave"
    />
  );
}

const set = { start: '2026-12-01', end: '2026-12-05' };

export function StateInvalid() {
  const stay = useLocal<CalendarRange>('taken', set);
  const isTaken = stay.value.start === set.start && stay.value.end === set.end;
  return <DateRangeInput label="Stay" value={stay.value} onChange={stay.set} format={short} locale={calendarEn} today={today} status={isTaken ? { type: 'error', message: 'These nights are taken.' } : undefined} data-testid="invalid" />;
}

export function StateDisabled() {
  return <DateRangeInput label="Stay" value={set} onChange={unused} format={short} locale={calendarEn} today={today} isDisabled data-testid="disabled" />;
}

/** The home tile. */
export function Specimen() {
  return <DateRangeInput label="Stay" isLabelHidden size="sm" value={set} onChange={unused} format={short} locale={calendarEn} today={today} className="w-36" />;
}
