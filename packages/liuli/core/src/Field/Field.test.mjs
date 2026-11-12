import { test } from 'node:test';
import assert from 'node:assert/strict';
import { textOf } from '@lingxia/view/testing';
import { renderExample, renderPage } from '../testing/harness.mjs';

test('WrappedSelect: the visible label is hidden from assistive tech, the control carries the name', async () => {
  const screen = await renderExample('Field/Field', 'WrappedSelect');
  const field = screen.getByTestId('field');
  const label = field.children[0];
  assert.equal(label.props.accessibilityHidden, 1);
  assert.equal(textOf(label), 'Billing period *');
  assert.equal(screen.getByTestId('period').props.accessibilityLabel, 'Billing period');
  assert.equal(textOf(screen.getByText('You can change it later.')), 'You can change it later.');
});

test('the status region stays mounted and empties when the status goes', async () => {
  // The testing screen cannot choose a select option, so a button sets the value.
  const screen = await renderPage(`
    import { useLocal } from '@lingxia/view';
    import { Field } from '@liulidesign/core/Field';
    export function render() {
      const period = useLocal('period', '');
      return <page>
        <Field label="Billing period" status={period.value ? undefined : { type: 'warning', message: 'Choose a period.' }}>
          <select bind={period} aria-label="Billing period" options={[{ value: 'yearly', label: 'Yearly' }]} />
        </Field>
        <button data-testid="choose" onClick={() => period.set('yearly')}>Choose</button>
      </page>;
    }
  `);
  const region = () => screen.queryAll(node => node.props.role === 'status');
  assert.equal(region().length, 1);
  assert.equal(textOf(region()[0]), 'Choose a period.');
  assert.ok(region()[0].children.some(node => node.tag === 'vector' && node.props.accessibilityHidden === 1), 'icon, hidden');
  screen.press(screen.getByTestId('choose'));
  assert.equal(region().length, 1, 'still mounted');
  assert.equal(textOf(region()[0]), '');
  assert.equal(region()[0].props.opacity, 0, 'visually hidden while empty');
});
