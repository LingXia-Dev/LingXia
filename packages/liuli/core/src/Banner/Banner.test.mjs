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

test('Collapsible: folded to its title, the toggle says whether it is open and brings the rest back', async () => {
  const screen = await renderExample('Banner/Banner', 'Collapsible');
  const texts = () => screen.getByTestId('maintenance').children[1].children.length;
  const toggle = () => screen.getByTestId('maintenance-toggle');
  assert.deepEqual([texts(), toggle().props.accessibilityLabel, toggle().props.stateExpanded], [1, 'Details', 0]);
  screen.press(toggle());
  assert.equal(texts(), 3, 'the message and the action');
  assert.equal(toggle().props.stateExpanded, 1);
});

test('the tone icon centres on the title\'s first line at any text size, with no offset of its own', async () => {
  const screen = await renderExample('Banner/Banner', 'Tones');
  const banner = screen.queryAll(node => node.props.role === 'status')[0];
  const [slot] = banner.children;
  assert.deepEqual([slot.props.alignSelf, slot.children[0].text, slot.children[0].props.lineHeight], ['start', '\u200B', 22]);
  assert.equal(slot.children[1].props.marginTop, undefined);
});
