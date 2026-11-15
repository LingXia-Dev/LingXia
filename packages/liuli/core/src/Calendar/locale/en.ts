import type { CalendarLocale } from '../Calendar';

const months = ['January', 'February', 'March', 'April', 'May', 'June', 'July', 'August', 'September', 'October', 'November', 'December'];
const days = ['Sunday', 'Monday', 'Tuesday', 'Wednesday', 'Thursday', 'Friday', 'Saturday'];

/** English, weeks from Monday. Spread it to change a word or the first day. */
export const calendarEn: CalendarLocale = {
  firstDay: 1,
  weekdays: ['S', 'M', 'T', 'W', 'T', 'F', 'S'],
  monthTitle: (year, month) => `${months[month - 1]} ${year}`,
  dayLabel: (year, month, day, weekday, isToday) => `${isToday ? 'Today, ' : ''}${days[weekday]} ${day} ${months[month - 1]} ${year}`,
  previousMonth: 'Previous month',
  nextMonth: 'Next month',
};
