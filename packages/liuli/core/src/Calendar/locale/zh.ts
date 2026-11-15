import type { CalendarLocale } from '../Calendar';

const days = ['星期日', '星期一', '星期二', '星期三', '星期四', '星期五', '星期六'];

/** 中文，每周从星期一开始。 */
export const calendarZh: CalendarLocale = {
  firstDay: 1,
  weekdays: ['日', '一', '二', '三', '四', '五', '六'],
  monthTitle: (year, month) => `${year}年${month}月`,
  dayLabel: (year, month, day, weekday, isToday) => `${isToday ? '今天，' : ''}${year}年${month}月${day}日${days[weekday]}`,
  previousMonth: '上个月',
  nextMonth: '下个月',
};
