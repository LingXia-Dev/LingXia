import { test } from 'node:test';
import assert from 'node:assert/strict';
import { renderExample } from '../testing/harness.mjs';

// The test environment's clock is 12-hour; the time follows it.

test('Activity: how long ago today, yesterday with its time, then the date, with the year only for another year', async () => {
  const screen = await renderExample('Timestamp/Timestamp', 'Activity');
  const shown = [0, 1, 2, 3, 4, 5].map(index => screen.getByTestId(`when-${index}`).text);
  assert.deepEqual(shown, ['Just now', '23 minutes ago', '2 hours ago', 'Yesterday, 6:30 PM', '2 November', '19 December 2025']);
});

test('Activity: a short form is read in full; digits keep their width', async () => {
  const when = (await renderExample('Timestamp/Timestamp', 'Activity')).getByTestId('when-1');
  assert.deepEqual([when.props.accessibilityLabel, when.props.fontVariantNumeric], ['14 November, 9:42 AM', 'tabular-nums']);
});

test('Formats: one moment in each form; the full form needs no second reading', async () => {
  const screen = await renderExample('Timestamp/Timestamp', 'Formats');
  const at = format => screen.getByTestId(`format-${format}`);
  assert.deepEqual(['relative', 'date', 'time', 'date-time'].map(format => at(format).text), ['Yesterday, 6:30 PM', '13 November', '6:30 PM', '13 November, 6:30 PM']);
  assert.equal(at('date-time').props.accessibilityLabel, undefined);
});

test('Chinese: a Chinese page writes the same moments in Chinese', async () => {
  const screen = await renderExample('Timestamp/Timestamp', 'Activity', { env: { language: 'zh-CN' } });
  assert.deepEqual([1, 3, 5].map(index => screen.getByTestId(`when-${index}`).text), ['23分钟前', '昨天 下午6:30', '2025年12月19日']);
});
