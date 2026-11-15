import { test } from 'node:test';
import assert from 'node:assert/strict';
import { textOf } from '@lingxia/view/testing';
import { renderExample } from '../testing/harness.mjs';

const items = screen => screen.queryAll(node => node.props.role === 'listitem');
const names = screen => items(screen).map(item => item.props.accessibilityLabel ?? item.children[0].props.accessibilityLabel);

test('Checkout: a named list; each step is read with its state, a finished one is a button that goes back', async () => {
  const screen = await renderExample('Stepper/Stepper', 'Checkout');
  const list = screen.getByTestId('checkout');
  assert.deepEqual([list.props.role, list.props.accessibilityLabel], ['list', 'Checkout steps']);
  assert.deepEqual(names(screen), ['Cart, Done', 'Address, Current step', 'Payment, Not started', 'Review, Not started']);
  assert.equal(screen.getByTestId('checkout-cart').tag, 'button');
  assert.notEqual(screen.getByTestId('checkout-address').tag, 'button', 'only finished steps go back');
  const marker = id => screen.getByTestId(`checkout-${id}`).children[0].children[0];
  assert.deepEqual([marker('cart').props.background, marker('cart').children[0].tag], ['accent', 'vector'], 'a tick on the finished step');
  assert.deepEqual([textOf(marker('address')), marker('address').children[0].props.color], ['2', 'accent']);
  assert.ok(marker('cart').props.accessibilityHidden && marker('payment').props.accessibilityHidden, 'the markers are not read');
  const line = id => screen.getByTestId(`checkout-${id}`).children[0].children[1];
  assert.deepEqual([line('cart').props.background, line('address').props.background, line('review')], ['accent', 'text/12', undefined], 'the line is filled behind, quiet ahead, absent after the last');
});

test('Checkout: moving on ticks the step left behind; going back makes the later ones not started again', async () => {
  const screen = await renderExample('Stepper/Stepper', 'Checkout');
  screen.press(screen.getByTestId('continue'));
  screen.press(screen.getByTestId('continue'));
  assert.deepEqual(names(screen), ['Cart, Done', 'Address, Done', 'Payment, Done', 'Review, Current step']);
  screen.press(screen.getByTestId('continue'));
  assert.deepEqual(names(screen).at(-1), 'Review, Done', 'past the last step every step is ticked');
  screen.press(screen.getByTestId('checkout-address'));
  assert.deepEqual(names(screen), ['Cart, Done', 'Address, Current step', 'Payment, Not started', 'Review, Not started']);
});

test('Across: horizontal steps share the row, the last taking only its own width, with lines between the markers', async () => {
  const screen = await renderExample('Stepper/Stepper', 'Across');
  assert.equal(screen.getByTestId('across').props.flexDirection, 'row');
  assert.deepEqual(items(screen).map(item => item.props.grow ?? 0), [1, 1, 0]);
  assert.deepEqual(names(screen), ['Account, Done', 'Profile, Current step', 'Done, Not started']);
  assert.equal(screen.queryAll(node => node.text === 'Where to deliver').length, 0);
});
