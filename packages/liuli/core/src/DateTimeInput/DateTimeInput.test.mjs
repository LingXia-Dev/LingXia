import { test } from 'node:test';
import assert from 'node:assert/strict';
import { renderExample } from '../testing/harness.mjs';

test('Reminder: one group named by the label, with a named date and time holding the parts of one value', async () => {
  const screen = await renderExample('DateTimeInput/DateTimeInput', 'Reminder');
  const group = screen.getByTestId('remind');
  assert.deepEqual([group.props.role, group.props.accessibilityLabel], ['group', 'Remind me']);
  const [date, time] = [screen.getByTestId('remind-date'), screen.getByTestId('remind-time')];
  assert.deepEqual([date.props.accessibilityLabel, date.props.inputValue, time.props.accessibilityLabel, time.props.inputValue], ['Reminder date', '2026-11-20', 'Reminder time', '09:00']);
  screen.type(time, '14:30');
  screen.getByText('Set for 2026-11-20 at 14:30');
});

test('Required: the value is whole only once both parts are set; one part is kept while the other is empty', async () => {
  const screen = await renderExample('DateTimeInput/DateTimeInput', 'Required');
  const part = id => screen.getByTestId(`starts-${id}`);
  screen.type(part('date'), '2026-11-21');
  assert.deepEqual([part('date').props.inputValue, part('time').props.inputValue], ['2026-11-21', '']);
  screen.getByText('Choose a day and a time.');
  screen.type(part('time'), '08:15');
  assert.equal(screen.queryAll(node => node.text === 'Choose a day and a time.').length, 0);
});

test('Reminder: the pair wraps by base widths; each part grows to fill its line and has no least width of its own', async () => {
  const screen = await renderExample('DateTimeInput/DateTimeInput', 'Reminder');
  const group = screen.getByTestId('remind');
  assert.deepEqual([group.props.flexDirection, group.props.flexWrap], ['row', 'wrap']);
  // Each part is its own field column inside the group.
  const [date, time] = group.children;
  assert.deepEqual([date.props.flexBasis, date.props.grow, time.props.flexBasis, time.props.grow], [160, 2, 112, 1]);
  assert.ok(date.props.flexBasis + time.props.flexBasis + group.props.gap > 240, 'under 280 pt the time takes its own row');
  // The View keeps an editor as wide as its widest value; a least width of the part's own would push it past a narrow container.
  assert.deepEqual([date.props.minWidth, time.props.minWidth], [undefined, undefined]);
});
