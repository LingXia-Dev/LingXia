/** A day as `YYYY-MM-DD`: it sorts as text and carries no time zone. */
export type CalendarDate = string;

const two = (value: number) => String(value).padStart(2, '0');

export const toDate = (year: number, month: number, day: number): CalendarDate => `${String(year).padStart(4, '0')}-${two(month)}-${two(day)}`;
/** `[year, month 1-12, day]`. */
export const parts = (date: CalendarDate): [number, number, number] => [Number(date.slice(0, 4)), Number(date.slice(5, 7)), Number(date.slice(8, 10))];
export const isDate = (text: string | null | undefined): text is CalendarDate => typeof text === 'string' && /^\d{4}-\d{2}-\d{2}$/.test(text);

const isLeap = (year: number) => (year % 4 === 0 && year % 100 !== 0) || year % 400 === 0;
const lengths = [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
export const daysIn = (year: number, month: number) => (month === 2 && isLeap(year) ? 29 : lengths[month - 1]);

const offsets = [0, 3, 2, 5, 0, 3, 5, 1, 4, 6, 2, 4];
/** 0 for Sunday to 6 for Saturday, by arithmetic: a View takes the time as a prop, never from a clock. */
export function weekdayOf(year: number, month: number, day: number): number {
  const y = month < 3 ? year - 1 : year;
  return (y + Math.floor(y / 4) - Math.floor(y / 100) + Math.floor(y / 400) + offsets[month - 1] + day) % 7;
}

/** The month `by` months on from `[year, month]`. */
export function shiftMonth(year: number, month: number, by: number): [number, number] {
  const index = year * 12 + (month - 1) + by;
  return [Math.floor(index / 12), (index % 12 + 12) % 12 + 1];
}

/** A month as weeks of seven days, `null` where a week reaches outside the month. */
export function weeksOf(year: number, month: number, firstDay: number): (number | null)[][] {
  const lead = (weekdayOf(year, month, 1) - firstDay + 7) % 7;
  const cells: (number | null)[] = [...Array<null>(lead).fill(null), ...Array.from({ length: daysIn(year, month) }, (_, index) => index + 1)];
  while (cells.length % 7) cells.push(null);
  return Array.from({ length: cells.length / 7 }, (_, week) => cells.slice(week * 7, week * 7 + 7));
}
