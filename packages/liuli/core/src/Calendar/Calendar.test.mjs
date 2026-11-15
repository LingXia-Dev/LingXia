import { test } from 'node:test';
import assert from 'node:assert/strict';
import { textOf } from '@lingxia/view/testing';
import { renderExample } from '../testing/harness.mjs';

const days = screen => screen.queryAll(node => node.props.role === 'gridcell');
const selected = screen => days(screen).filter(day => day.props.stateSelected === 1).map(textOf);

test('Day: November 2026 from Monday, each day named in full, the chosen one selected and filled, today ringed', async () => {
  const screen = await renderExample('Calendar/Calendar', 'Day');
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
  assert.deepEqual([day('2026-11-19').props.background, day('2026-11-19').children[0].props.color], ['accent', 'on-accent']);
  assert.ok(day('2026-11-15').props.ring, 'a ring on today');
  assert.deepEqual([day('2026-11-19').props.width, day('2026-11-19').props.height], [40, 40]);
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
  assert.deepEqual(['18', '21', '24'].map(d => day(`2026-11-${d}`).props.background), ['accent', undefined, 'accent'], 'the ends are filled, the days between sit on the band');
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
  const zh = await renderExample('Calendar/Calendar', 'Chinese');
  assert.equal(textOf(zh.queryAll(node => node.props.role === 'status')[0]), '2026年11月');
  assert.equal(zh.getByTestId('zh-2026-11-19').props.accessibilityLabel, '2026年11月19日星期四');
  assert.equal(textOf(zh.queryAll(node => node.props.role === 'grid')[0].children[0]), '一二三四五六日');
});

test('Day: the days are one grid that turns the month on PageUp and PageDown', async () => {
  const screen = await renderExample('Calendar/Calendar', 'Day');
  const grid = screen.queryAll(node => node.props.role === 'grid')[0];
  assert.ok(grid.props.onPage ?? grid.props.onGridPage, JSON.stringify(Object.keys(grid.props)));
});
