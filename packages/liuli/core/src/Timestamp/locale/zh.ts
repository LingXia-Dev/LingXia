import type { TimestampLocale } from '../Timestamp';

const two = (n: number) => String(n).padStart(2, '0');

/** 简体中文。展开它可改动个别用语。 */
export const timestampZh: TimestampLocale = {
  justNow: '刚刚',
  minutesAgo: minutes => `${minutes} 分钟前`,
  hoursAgo: hours => `${hours} 小时前`,
  yesterday: '昨天',
  date: (year, month, day, hasYear) => `${hasYear ? `${year}年` : ''}${month}月${day}日`,
  time: (hour, minute, isTwelveHour) => (isTwelveHour ? `${hour < 12 ? '上午' : '下午'}${hour % 12 || 12}:${two(minute)}` : `${two(hour)}:${two(minute)}`),
  dateTime: (date, time) => `${date} ${time}`,
};
