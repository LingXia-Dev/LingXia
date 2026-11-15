import { test } from 'node:test';
import assert from 'node:assert/strict';
import { renderExample } from '../testing/harness.mjs';

test('Countdown: minutes and two-digit seconds in steady digits, read in words, muted at zero', async () => {
  const screen = await renderExample('Timer/Timer', 'Countdown');
  const timer = () => screen.getByTestId('countdown');
  assert.deepEqual([timer().text, timer().props.accessibilityLabel, timer().props.fontVariantNumeric, timer().props.color], ['2:05', '2 minutes 5 seconds left', 'tabular-nums', 'text']);
  for (let i = 0; i < 9; i++) screen.press(screen.getByTestId('skip'));
  assert.deepEqual([timer().text, timer().props.color], ['0:00', 'text-muted']);
});

test('Inline: hours appear from an hour on', async () => {
  const screen = await renderExample('Timer/Timer', 'Inline');
  assert.deepEqual([screen.getByTestId('expires').text, screen.getByTestId('call').text], ['4:32', '1:12:05']);
});
