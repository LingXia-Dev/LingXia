import type { TimestampLocale } from '../Timestamp';

const months = ['January', 'February', 'March', 'April', 'May', 'June', 'July', 'August', 'September', 'October', 'November', 'December'];
const two = (n: number) => String(n).padStart(2, '0');

/** English, day before month. Spread it to change a phrase. */
export const timestampEn: TimestampLocale = {
  justNow: 'Just now',
  minutesAgo: minutes => (minutes === 1 ? '1 minute ago' : `${minutes} minutes ago`),
  hoursAgo: hours => (hours === 1 ? '1 hour ago' : `${hours} hours ago`),
  yesterday: 'Yesterday',
  date: (year, month, day, hasYear) => `${day} ${months[month - 1]}${hasYear ? ` ${year}` : ''}`,
  time: (hour, minute, isTwelveHour) => (isTwelveHour ? `${hour % 12 || 12}:${two(minute)} ${hour < 12 ? 'AM' : 'PM'}` : `${two(hour)}:${two(minute)}`),
  dateTime: (date, time) => `${date}, ${time}`,
};
