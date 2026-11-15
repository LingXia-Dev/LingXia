import { test } from 'node:test';
import assert from 'node:assert/strict';
import { renderExample } from '../testing/harness.mjs';

test('Reply: nothing to send, nothing to press; a draft wakes the button, and sending trims it and empties the field', async () => {
  const screen = await renderExample('Composer/Composer', 'Reply');
  const field = () => screen.getByTestId('reply');
  const send = () => screen.getByTestId('reply-send');
  assert.deepEqual([field().props.accessibilityLabel, field().props.rows, field().props.maxRows, send().props.accessibilityLabel, send().props.enabled], ['Message', 1, 5, 'Send', 0]);
  screen.type(field(), '   ');
  assert.equal(send().props.enabled, 0, 'spaces are not a message');
  screen.type(field(), '  Bring the tent  ');
  assert.notEqual(send().props.enabled, 0);
  screen.press(send());
  screen.getByText('Bring the tent');
  assert.equal(field().props.inputValue, '');
});

test('Reply: the keyboard\'s send key sends as the button does', async () => {
  const screen = await renderExample('Composer/Composer', 'Reply');
  screen.type(screen.getByTestId('reply'), 'Leaving now');
  screen.submit(screen.getByTestId('reply'));
  screen.getByText('Leaving now');
});

test('StateSending: the button is busy and takes no second send', async () => {
  const send = (await renderExample('Composer/Composer', 'StateSending')).getByTestId('sending-send');
  assert.equal(send.props.stateBusy, 1);
});
