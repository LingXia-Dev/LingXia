import { test } from 'node:test';
import assert from 'node:assert/strict';
import { renderExample } from '../testing/harness.mjs';

test('Payment: a status region with the action and a named dismiss button; dismissing removes it', async () => {
  const screen = await renderExample('Banner/Banner', 'Payment');
  assert.equal(screen.getByTestId('payment').props.role, 'status');
  screen.press(screen.getByTestId('payment-action'));
  screen.getByText('Card updated');
  const dismiss = screen.getByTestId('payment-dismiss');
  assert.equal(dismiss.props.accessibilityLabel, 'Dismiss');
  screen.press(dismiss);
  assert.equal(screen.queryAll(node => node.testId === 'payment').length, 0);
});

test('OverList: a danger banner is an alert', async () => {
  const screen = await renderExample('Banner/Banner', 'OverList');
  assert.equal(screen.getByTestId('sync').props.role, 'alert');
  screen.press(screen.getByTestId('sync-action'));
  screen.getByText('Still offline');
});
