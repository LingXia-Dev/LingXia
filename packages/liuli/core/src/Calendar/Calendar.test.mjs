import { test } from 'node:test';
import assert from 'node:assert/strict';
import { textOf } from '@lingxia/view/testing';
import { renderExample, renderPage } from '../testing/harness.mjs';

const days = screen => screen.queryAll(node => node.props.role === 'gridcell');
const selected = screen => days(screen).filter(day => day.props.stateSelected === 1).map(textOf);

test('Day: November 2026 from Monday, each day named in full, the chosen one selected and filled, today ringed', async () => {
  const screen = await renderExample('Calendar/Calendar', 'Day', { env: { firstWeekday: 1 } });
  const calendar = screen.getByTestId('due');
  assert.deepEqual([calendar.props.role, calendar.props.accessibilityLabel], ['group', 'Due date']);
  const grid = screen.queryAll(node => node.props.role === 'grid')[0];
  assert.equal(grid.props.accessibilityLabel, 'November 2026');
  const [names, ...weeks] = grid.children;
  assert.equal(names.props.accessibilityHidden, 1);
  assert.equal(textOf(names), 'MTWTFSS');
  assert.equal(weeks.length, 6, 'thirty days from a Sunday, in weeks from Monday');
  assert.deepEqual(weeks[0].children.map(slot => textOf(slot)), ['', '', '', '', '', '', '1'], '1 November 2026 is a Sunday');
  assert.equal(days(screen).length, 30);
  const day = date => screen.getByTestId(`due-${date}`);
  assert.equal(day('2026-11-19').props.accessibilityLabel, 'Thursday 19 November 2026');
  assert.equal(day('2026-11-15').props.accessibilityLabel, 'Today, Sunday 15 November 2026');
  assert.deepEqual(selected(screen), ['19']);
  assert.deepEqual([day('2026-11-19').props.background, day('2026-11-19').children[0].props.color], ['accent-solid', 'on-accent-solid']);
  assert.ok(day('2026-11-15').props.ring, 'a ring on today');
  assert.deepEqual([day('2026-11-19').props.minWidth, day('2026-11-19').props.minHeight], [40, 40]);
  // At large text a two-digit day widens its circle, which stays square and inside its seventh of the row.
  assert.deepEqual([day('2026-11-19').props.width, day('2026-11-19').props.aspectRatio, day('2026-11-19').props.maxWidthPercent, day('2026-11-19').props.paddingLeft], [undefined, 1, 100, 4]);
  assert.equal(day('2026-11-19').parent.props.height, undefined, 'the week row grows with it');
  screen.press(day('2026-11-03'));
  assert.deepEqual(selected(screen), ['3']);
});

test('Day: the month buttons move a month at a time and the title, a status, follows', async () => {
  const screen = await renderExample('Calendar/Calendar', 'Day');
  const title = () => screen.queryAll(node => node.props.role === 'status')[0];
  assert.equal(textOf(title()), 'November 2026');
  assert.deepEqual(['due-previous', 'due-next'].map(id => screen.getByTestId(id).props.accessibilityLabel), ['Previous month', 'Next month']);
  screen.press(screen.getByTestId('due-next'));
  screen.press(screen.getByTestId('due-next'));
  assert.equal(textOf(title()), 'January 2027');
  assert.equal(days(screen).length, 31);
  assert.deepEqual(selected(screen), [], 'the chosen day is in another month');
  screen.press(screen.getByTestId('due-previous'));
  assert.equal(textOf(title()), 'December 2026');
});

test('Stay: a span is selected end to end with a band between; a press starts over, the next ends it, earlier day first', async () => {
  const screen = await renderExample('Calendar/Calendar', 'Stay');
  const day = date => screen.getByTestId(`stay-${date}`);
  assert.deepEqual(selected(screen), ['18', '19', '20', '21', '22', '23', '24']);
  assert.deepEqual(['18', '21', '24'].map(d => day(`2026-11-${d}`).parent.props.background), ['accent-soft', 'accent-soft', 'accent-soft']);
  assert.deepEqual(['18', '21', '24'].map(d => day(`2026-11-${d}`).props.background), ['accent-solid', undefined, 'accent-solid'], 'the ends are filled, the days between sit on the band');
  screen.press(day('2026-11-27'));
  assert.deepEqual(selected(screen), ['27']);
  assert.equal(day('2026-11-27').parent.props.background, undefined, 'one day has no band');
  screen.getByText('2026-11-27 – ');
  screen.press(day('2026-11-25'));
  assert.deepEqual(selected(screen), ['25', '26', '27']);
  screen.getByText('2026-11-25 – 2026-11-27');
  assert.equal(day('2026-11-14').props.onClick, undefined, 'a day before the minimum cannot be chosen');
  assert.equal(screen.getByTestId('stay-previous').props.onClick, undefined, 'nor a month that ends before it be opened');
});

test('Weekdays and Chinese: ruled-out days are disabled in place; the locale gives the words and the week\'s first day', async () => {
  const screen = await renderExample('Calendar/Calendar', 'Weekdays');
  assert.equal(textOf(screen.queryAll(node => node.props.role === 'status')[0]), 'December 2026', 'a controlled month');
  assert.equal(screen.getByTestId('appointment-2026-12-05').props.onClick, undefined, 'a Saturday');
  assert.notEqual(screen.getByTestId('appointment-2026-12-07').props.onClick, undefined, 'a Monday');
  assert.equal(screen.getByTestId('appointment-2026-12-06').props.onClick, undefined, 'a Sunday');
  assert.notEqual(screen.getByTestId('appointment-2026-12-04').props.onClick, undefined, 'a Friday');
  // The words follow the page's language; the week starts on the user's first weekday (1 = Monday).
  const zh = await renderExample('Calendar/Calendar', 'Day', { env: { language: 'zh-CN', firstWeekday: 1 } });
  assert.equal(textOf(zh.queryAll(node => node.props.role === 'status')[0]), '2026年11月');
  assert.equal(zh.getByTestId('due-2026-11-19').props.accessibilityLabel, '2026年11月19日星期四');
  assert.equal(textOf(zh.queryAll(node => node.props.role === 'grid')[0].children[0]), '一二三四五六日');
});

test('Day: the days are one grid that turns the month on PageUp and PageDown', async () => {
  const screen = await renderExample('Calendar/Calendar', 'Day');
  const grid = screen.queryAll(node => node.props.role === 'grid')[0];
  assert.ok(grid.props.onPage ?? grid.props.onGridPage, JSON.stringify(Object.keys(grid.props)));
});

test('Day: the week starts on the user\'s first weekday, Sunday when unset, and never moves with the words alone', async () => {
  const initials = async env => textOf((await renderExample('Calendar/Calendar', 'Day', { env })).queryAll(node => node.props.role === 'grid')[0].children[0]);
  assert.equal(await initials({}), 'SMTWTFS', 'the default environment, en-US: Sunday');
  assert.equal(await initials({ firstWeekday: 1 }), 'MTWTFSS', 'a Monday-first user');
  assert.equal(await initials({ firstWeekday: 6 }), 'SSMTWTF', 'a Saturday-first user');
  assert.equal(await initials({ language: 'zh-CN' }), '日一二三四五六', 'the words do not move the week: the user\'s first weekday does');
});

test('MondayFirst: the app\'s own week wins over the user\'s first weekday', async () => {
  const initials = async env => textOf((await renderExample('Calendar/Calendar', 'MondayFirst', { env })).queryAll(node => node.props.role === 'grid')[0].children[0]);
  assert.equal(await initials({}), 'MTWTFSS');
  assert.equal(await initials({ firstWeekday: 7 }), 'MTWTFSS', 'a Sunday-first user');
  assert.equal(await initials({ firstWeekday: 6 }), 'MTWTFSS', 'a Saturday-first user');
});

test('a swipe or key on the grid does not page past min or max, as the buttons do not', async () => {
  const screen = await renderPage(`
    import { useLocal } from '@lingxia/view';
    import { Calendar } from '@liulidesign/core/Calendar';
    export function render() {
      const day = useLocal('day', '2026-11-20');
      return <page><Calendar label="Day" bind={day} min="2026-11-01" max="2026-12-31" today="2026-11-20" data-testid="day" /></page>;
    }
  `);
  const grid = () => screen.queryAll(node => node.props.role === 'grid')[0];
  screen.fire(grid(), 'onPage', 'previous');
  assert.equal(grid().props.accessibilityLabel, 'November 2026', 'nothing before min to page to');
  screen.fire(grid(), 'onPage', 'next');
  screen.fire(grid(), 'onPage', 'next');
  assert.equal(grid().props.accessibilityLabel, 'December 2026', 'nothing after max');
});
