// Calendar arithmetic and names for the View's own date and time pickers. The runtime has no
// Intl, so names come from these tables; other languages read English names with their own
// week start and clock from `env`.

const names = {
  en: { months: ['January', 'February', 'March', 'April', 'May', 'June', 'July', 'August', 'September', 'October', 'November', 'December'],
    days: ['Sun', 'Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat'], longDays: ['Sunday', 'Monday', 'Tuesday', 'Wednesday', 'Thursday', 'Friday', 'Saturday'],
    title: (month, year) => `${month} ${year}`, full: (day, month, year, weekday) => `${weekday}, ${month} ${day}, ${year}`,
    am: 'AM', pm: 'PM', cancel: 'Cancel', done: 'Done', previous: 'Previous month', next: 'Next month', hour: 'Hour', minute: 'Minute', period: 'AM/PM' },
  zh: { months: ['1月', '2月', '3月', '4月', '5月', '6月', '7月', '8月', '9月', '10月', '11月', '12月'],
    days: ['日', '一', '二', '三', '四', '五', '六'], longDays: ['星期日', '星期一', '星期二', '星期三', '星期四', '星期五', '星期六'],
    title: (month, year) => `${year}年${month}`, full: (day, month, year, weekday) => `${year}年${month}${day}日${weekday}`,
    am: '上午', pm: '下午', cancel: '取消', done: '完成', previous: '上个月', next: '下个月', hour: '时', minute: '分', period: '上午/下午' },
  ja: { months: ['1月', '2月', '3月', '4月', '5月', '6月', '7月', '8月', '9月', '10月', '11月', '12月'],
    days: ['日', '月', '火', '水', '木', '金', '土'], longDays: ['日曜日', '月曜日', '火曜日', '水曜日', '木曜日', '金曜日', '土曜日'],
    title: (month, year) => `${year}年${month}`, full: (day, month, year, weekday) => `${year}年${month}${day}日 ${weekday}`,
    am: '午前', pm: '午後', cancel: 'キャンセル', done: '完了', previous: '前の月', next: '次の月', hour: '時', minute: '分', period: '午前/午後' },
  ko: { months: ['1월', '2월', '3월', '4월', '5월', '6월', '7월', '8월', '9월', '10월', '11월', '12월'],
    days: ['일', '월', '화', '수', '목', '금', '토'], longDays: ['일요일', '월요일', '화요일', '수요일', '목요일', '금요일', '토요일'],
    title: (month, year) => `${year}년 ${month}`, full: (day, month, year, weekday) => `${year}년 ${month} ${day}일 ${weekday}`,
    am: '오전', pm: '오후', cancel: '취소', done: '완료', previous: '이전 달', next: '다음 달', hour: '시', minute: '분', period: '오전/오후' },
  de: { months: ['Januar', 'Februar', 'März', 'April', 'Mai', 'Juni', 'Juli', 'August', 'September', 'Oktober', 'November', 'Dezember'],
    days: ['So', 'Mo', 'Di', 'Mi', 'Do', 'Fr', 'Sa'], longDays: ['Sonntag', 'Montag', 'Dienstag', 'Mittwoch', 'Donnerstag', 'Freitag', 'Samstag'],
    title: (month, year) => `${month} ${year}`, full: (day, month, year, weekday) => `${weekday}, ${day}. ${month} ${year}`,
    am: 'AM', pm: 'PM', cancel: 'Abbrechen', done: 'Fertig', previous: 'Vorheriger Monat', next: 'Nächster Monat', hour: 'Stunde', minute: 'Minute', period: 'AM/PM' },
  fr: { months: ['janvier', 'février', 'mars', 'avril', 'mai', 'juin', 'juillet', 'août', 'septembre', 'octobre', 'novembre', 'décembre'],
    days: ['dim.', 'lun.', 'mar.', 'mer.', 'jeu.', 'ven.', 'sam.'], longDays: ['dimanche', 'lundi', 'mardi', 'mercredi', 'jeudi', 'vendredi', 'samedi'],
    title: (month, year) => `${month} ${year}`, full: (day, month, year, weekday) => `${weekday} ${day} ${month} ${year}`,
    am: 'AM', pm: 'PM', cancel: 'Annuler', done: 'OK', previous: 'Mois précédent', next: 'Mois suivant', hour: 'Heure', minute: 'Minute', period: 'AM/PM' },
  es: { months: ['enero', 'febrero', 'marzo', 'abril', 'mayo', 'junio', 'julio', 'agosto', 'septiembre', 'octubre', 'noviembre', 'diciembre'],
    days: ['dom', 'lun', 'mar', 'mié', 'jue', 'vie', 'sáb'], longDays: ['domingo', 'lunes', 'martes', 'miércoles', 'jueves', 'viernes', 'sábado'],
    title: (month, year) => `${month} de ${year}`, full: (day, month, year, weekday) => `${weekday}, ${day} de ${month} de ${year}`,
    am: 'a. m.', pm: 'p. m.', cancel: 'Cancelar', done: 'Listo', previous: 'Mes anterior', next: 'Mes siguiente', hour: 'Hora', minute: 'Minuto', period: 'a. m./p. m.' },
  pt: { months: ['janeiro', 'fevereiro', 'março', 'abril', 'maio', 'junho', 'julho', 'agosto', 'setembro', 'outubro', 'novembro', 'dezembro'],
    days: ['dom.', 'seg.', 'ter.', 'qua.', 'qui.', 'sex.', 'sáb.'], longDays: ['domingo', 'segunda-feira', 'terça-feira', 'quarta-feira', 'quinta-feira', 'sexta-feira', 'sábado'],
    title: (month, year) => `${month} de ${year}`, full: (day, month, year, weekday) => `${weekday}, ${day} de ${month} de ${year}`,
    am: 'AM', pm: 'PM', cancel: 'Cancelar', done: 'OK', previous: 'Mês anterior', next: 'Próximo mês', hour: 'Hora', minute: 'Minuto', period: 'AM/PM' },
  ru: { months: ['январь', 'февраль', 'март', 'апрель', 'май', 'июнь', 'июль', 'август', 'сентябрь', 'октябрь', 'ноябрь', 'декабрь'],
    days: ['вс', 'пн', 'вт', 'ср', 'чт', 'пт', 'сб'], longDays: ['воскресенье', 'понедельник', 'вторник', 'среда', 'четверг', 'пятница', 'суббота'],
    title: (month, year) => `${month} ${year}`, full: (day, month, year, weekday) => `${weekday}, ${day} ${month} ${year}`,
    am: 'AM', pm: 'PM', cancel: 'Отмена', done: 'Готово', previous: 'Предыдущий месяц', next: 'Следующий месяц', hour: 'Час', minute: 'Минута', period: 'AM/PM' },
  ar: { months: ['يناير', 'فبراير', 'مارس', 'أبريل', 'مايو', 'يونيو', 'يوليو', 'أغسطس', 'سبتمبر', 'أكتوبر', 'نوفمبر', 'ديسمبر'],
    days: ['أحد', 'إثنين', 'ثلاثاء', 'أربعاء', 'خميس', 'جمعة', 'سبت'], longDays: ['الأحد', 'الإثنين', 'الثلاثاء', 'الأربعاء', 'الخميس', 'الجمعة', 'السبت'],
    title: (month, year) => `${month} ${year}`, full: (day, month, year, weekday) => `${weekday}، ${day} ${month} ${year}`,
    am: 'ص', pm: 'م', cancel: 'إلغاء', done: 'تم', previous: 'الشهر السابق', next: 'الشهر التالي', hour: 'الساعة', minute: 'الدقيقة', period: 'ص/م' },
};

/** Picker words for a BCP 47 display language; English when it has no table. */
export function wordsFor(language) {
  return names[String(language ?? '').split(/[-_]/)[0].toLowerCase()] ?? names.en;
}

const pad = (value, width = 2) => String(value).padStart(width, '0');
const isLeap = year => year % 4 === 0 && (year % 100 !== 0 || year % 400 === 0);
export const daysIn = (year, month) => [31, isLeap(year) ? 29 : 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31][month - 1];

/** 0 = Sunday for a proleptic Gregorian date. */
export function weekday(year, month, day) {
  const t = [0, 3, 2, 5, 0, 3, 5, 1, 4, 6, 2, 4];
  const y = month < 3 ? year - 1 : year;
  return (y + Math.floor(y / 4) - Math.floor(y / 100) + Math.floor(y / 400) + t[month - 1] + day) % 7;
}

export const iso = ({ year, month, day }) => `${pad(year, 4)}-${pad(month)}-${pad(day)}`;
export function parseDate(value) {
  const match = /^(\d{4})-(\d{2})-(\d{2})$/.exec(value ?? '');
  if (!match) return null;
  const [year, month, day] = match.slice(1).map(Number);
  return month >= 1 && month <= 12 && day >= 1 && day <= daysIn(year, month) ? { year, month, day } : null;
}
export function parseTime(value) {
  const match = /^(\d{2}):(\d{2})(?::\d{2})?$/.exec(value ?? '');
  if (!match) return null;
  const [hour, minute] = match.slice(1).map(Number);
  return hour < 24 && minute < 60 ? { hour, minute } : null;
}

/** The date `days` after `date` (negative before). */
export function addDays({ year, month, day }, days) {
  let ordinal = toOrdinal(year, month, day) + days;
  return fromOrdinal(ordinal);
}
/** The same day `months` later, clamped to the month's length. */
export function addMonths({ year, month, day }, months) {
  const index = year * 12 + (month - 1) + months;
  const y = Math.floor(index / 12), m = index - y * 12 + 1;
  return { year: y, month: m, day: Math.min(day, daysIn(y, m)) };
}
function toOrdinal(year, month, day) {
  // Days since 0000-03-01, for whole-day arithmetic without Date.
  const y = month <= 2 ? year - 1 : year, m = month <= 2 ? month + 9 : month - 3;
  return 365 * y + Math.floor(y / 4) - Math.floor(y / 100) + Math.floor(y / 400) + Math.floor((153 * m + 2) / 5) + day - 1;
}
function fromOrdinal(ordinal) {
  let y = Math.floor((10000 * ordinal + 14780) / 3652425);
  let rest = ordinal - (365 * y + Math.floor(y / 4) - Math.floor(y / 100) + Math.floor(y / 400));
  if (rest < 0) { y -= 1; rest = ordinal - (365 * y + Math.floor(y / 4) - Math.floor(y / 100) + Math.floor(y / 400)); }
  const mi = Math.floor((100 * rest + 52) / 3060);
  const month = (mi + 2) % 12 + 1, year = y + Math.floor((mi + 2) / 12), day = rest - Math.floor((mi * 306 + 5) / 10) + 1;
  return { year, month, day };
}
export const compare = (a, b) => (a.year - b.year) || (a.month - b.month) || (a.day - b.day);

/**
 * Six weeks of cells for the month of `view`, starting on `firstWeekday` (1 = Sunday … 7 = Saturday):
 * `{date, inMonth}` each, so every month draws the same grid height.
 */
export function monthGrid({ year, month }, firstWeekday = 1) {
  const lead = (weekday(year, month, 1) - (firstWeekday - 1) + 7) % 7;
  const start = addDays({ year, month, day: 1 }, -lead);
  return Array.from({ length: 42 }, (_, index) => {
    const date = addDays(start, index);
    return { date, inMonth: date.month === month && date.year === year };
  });
}

/** Weekday indexes (0 = Sunday) in column order. */
export const weekdayOrder = firstWeekday => Array.from({ length: 7 }, (_, index) => (firstWeekday - 1 + index) % 7);

/** The date a key moves `date` to, or null for a key the grid does not use. */
export function moveDate(date, key, rtl = false) {
  switch (key) {
    case 'ArrowLeft': return addDays(date, rtl ? 1 : -1);
    case 'ArrowRight': return addDays(date, rtl ? -1 : 1);
    case 'ArrowUp': return addDays(date, -7);
    case 'ArrowDown': return addDays(date, 7);
    case 'PageUp': return addMonths(date, -1);
    case 'PageDown': return addMonths(date, 1);
    default: return null;
  }
}

export const within = (date, min, max) => (!min || compare(date, min) >= 0) && (!max || compare(date, max) <= 0);
export const clampDate = (date, min, max) => min && compare(date, min) < 0 ? min : max && compare(date, max) > 0 ? max : date;

/** Hour labels and values for a column: 0–23, or 1–12 with the day period separate. */
export function hourValues(cycle) {
  return cycle === 'h12' ? [12, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11] : Array.from({ length: 24 }, (_, hour) => hour);
}
/** Minutes a `step` (seconds) allows; at least one minute apart. */
export function minuteValues(step) {
  const every = Number.isFinite(step) && step >= 60 ? Math.max(1, Math.round(step / 60)) : 1;
  return Array.from({ length: Math.ceil(60 / every) }, (_, index) => index * every).filter(minute => minute < 60);
}
export const timeIso = ({ hour, minute }) => `${pad(hour)}:${pad(minute)}`;
