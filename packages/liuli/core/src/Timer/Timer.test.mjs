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

test('At large text: the digits stay on one line, and the rows around them wrap', async () => {
  const countdown = await renderExample('Timer/Timer', 'Countdown');
  assert.equal(countdown.getByTestId('countdown').props.whiteSpace, 'nowrap');
  const skip = countdown.getByTestId('skip');
  assert.equal(countdown.queryAll(node => node.children.includes(skip))[0].props.flexWrap, 'wrap', 'Reset moves under Skip rather than past the card');
  const inline = await renderExample('Timer/Timer', 'Inline');
  for (const id of ['expires', 'call']) {
    const timer = inline.getByTestId(id);
    assert.deepEqual([timer.props.whiteSpace, inline.queryAll(node => node.children.includes(timer))[0].props.flexWrap], ['nowrap', 'wrap'], id);
  }
});
