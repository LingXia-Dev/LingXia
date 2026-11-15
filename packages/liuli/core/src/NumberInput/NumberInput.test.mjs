import { test } from 'node:test';
import assert from 'node:assert/strict';
import { textOf } from '@lingxia/view/testing';
import { described, renderExample, renderPage } from '../testing/harness.mjs';

const value = (screen, id) => screen.getByTestId(id).props.inputValue;
const enabled = node => node.props.onClick !== undefined;

test('Guests: a number editor named by its label; the steppers are named buttons that move one step and stop at the ends', async () => {
  const screen = await renderExample('NumberInput/NumberInput', 'Guests');
  const field = screen.getByTestId('guests');
  assert.equal(field.tag, 'input');
  assert.equal(field.props.accessibilityLabel, 'Guests');
  assert.equal(field.props.fontVariantNumeric, 'tabular-nums');
  assert.deepEqual(described(screen, field), ['Up to 12 at one table.']);
  assert.equal(value(screen, 'guests'), '2');
  const [less, more] = ['guests-decrement', 'guests-increment'].map(id => () => screen.getByTestId(id));
  assert.deepEqual([less().props.accessibilityLabel, more().props.accessibilityLabel], ['Fewer guests', 'More guests']);
  assert.deepEqual([less().props.width, more().props.width], [40, 40]);
  screen.press(more());
  assert.equal(value(screen, 'guests'), '3');
  screen.press(less());
  screen.press(less());
  assert.equal(value(screen, 'guests'), '1');
  assert.equal(enabled(less()), false, 'at the minimum the minus stepper is disabled');
  for (let press = 0; press < 11; press += 1) screen.press(more());
  assert.equal(value(screen, 'guests'), '12');
  assert.equal(enabled(more()), false);
});

test('typing: the field keeps the text while it is on its way to a number, and carries a number or null', async () => {
  const screen = await renderPage(`
    import { useLocal } from '@lingxia/view';
    import { NumberInput } from '@liulidesign/core/NumberInput';
    export function render() {
      const n = useLocal('n', null);
      return <page><NumberInput label="Amount" bind={n} decrementLabel="Less" incrementLabel="More" data-testid="n" /><text data-testid="out">{JSON.stringify(n.value)}</text></page>;
    }
  `);
  const out = () => textOf(screen.getByTestId('out'));
  const type = text => screen.type(screen.getByTestId('n'), text);
  assert.equal(out(), 'null');
  type('-');
  assert.deepEqual([value(screen, 'n'), out()], ['-', 'null'], 'a lone minus is kept, and is not a number yet');
  type('-4');
  assert.deepEqual([value(screen, 'n'), out()], ['-4', '-4']);
  type('-4.');
  assert.deepEqual([value(screen, 'n'), out()], ['-4.', '-4'], 'the point on the way to a decimal is kept');
  type('-4.5');
  assert.equal(out(), '-4.5');
  screen.press(screen.getByTestId('n-increment'));
  assert.deepEqual([value(screen, 'n'), out()], ['-3.5', '-3.5'], 'a stepper replaces what was typed');
  type('');
  assert.deepEqual([value(screen, 'n'), out()], ['', 'null']);
  screen.press(screen.getByTestId('n-increment'));
  assert.equal(out(), '0', 'from empty, a stepper starts at zero when there is no minimum');
});

test('Weight: steps of a tenth stay exact, and the unit is read with the description', async () => {
  const screen = await renderExample('NumberInput/NumberInput', 'Weight');
  for (let press = 0; press < 3; press += 1) screen.press(screen.getByTestId('weight-increment'));
  assert.equal(value(screen, 'weight'), '71.8', 'no 71.80000000000001');
  assert.deepEqual(described(screen, screen.getByTestId('weight')), ['kg']);
  assert.equal(textOf(screen.getByTestId('weight').parent).includes('kg'), true);
});

test('Price: no steppers, a currency sign before the number, and the optional word read first', async () => {
  const screen = await renderExample('NumberInput/NumberInput', 'Price');
  assert.equal(screen.queryAll(node => node.tag === 'button').length, 0);
  assert.deepEqual(described(screen, screen.getByTestId('price')), ['$', 'optional']);
  assert.equal(screen.getByTestId('price').props.placeholder, '0.00');
});

test('states: the sizes; an error marks the edge and is read; disabled turns off the editor and both steppers', async () => {
  const sizes = await renderExample('NumberInput/NumberInput', 'Sizes');
  assert.deepEqual([sizes.getByTestId('small').parent.props.minHeight, sizes.getByTestId('large').parent.props.minHeight], [32, 48]);
  assert.deepEqual([sizes.getByTestId('small-increment').props.width, sizes.getByTestId('large-increment').props.width], [32, 48]);
  const invalid = await renderExample('NumberInput/NumberInput', 'StateInvalid');
  assert.equal(invalid.getByTestId('invalid').parent.props.borderColor, 'danger');
  assert.deepEqual(described(invalid, invalid.getByTestId('invalid')), ['Only 9 left in stock.']);
  const disabled = await renderExample('NumberInput/NumberInput', 'StateDisabled');
  assert.equal(disabled.getByTestId('disabled').props.enabled, 0);
  assert.deepEqual(['disabled-decrement', 'disabled-increment'].map(id => enabled(disabled.getByTestId(id))), [false, false]);
  const atMinimum = await renderExample('NumberInput/NumberInput', 'StateAtMinimum');
  assert.deepEqual(['at-minimum-decrement', 'at-minimum-increment'].map(id => enabled(atMinimum.getByTestId(id))), [false, true]);
});
