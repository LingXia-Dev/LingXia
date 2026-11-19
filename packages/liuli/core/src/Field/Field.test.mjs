import { test } from 'node:test';
import assert from 'node:assert/strict';
import { textOf } from '@lingxia/view/testing';
import { described, renderExample, renderPage } from '../testing/harness.mjs';

test('WrappedSelect: the visible label is hidden from assistive tech, the control carries the name', async () => {
  const screen = await renderExample('Field/Field', 'WrappedSelect');
  const field = screen.getByTestId('field');
  const label = field.children[0];
  assert.equal(label.props.accessibilityHidden, 1);
  assert.equal(textOf(label), 'Billing period *');
  const select = screen.getByTestId('period');
  assert.equal(select.props.accessibilityLabel, 'Billing period');
  assert.deepEqual(described(screen, select), ['You can change it later.', 'Choose a period to see the price.']);
  assert.equal(select.props.stateRequired, 1);
  assert.equal(select.props.stateInvalid, undefined, 'a warning is not an error');
});

test('the status region stays mounted and empties when the status goes', async () => {
  const screen = await renderPage(`
    import { useLocal } from '@lingxia/view';
    import { Field } from '@liulidesign/core/Field';
    export function render() {
      const period = useLocal('period', '');
      return <page>
        <Field label="Billing period" status={period.value ? undefined : { type: 'error', message: 'Choose a period.' }}>
          {control => <select bind={period} aria-label={control['aria-label']} aria-describedby={control['aria-describedby']}
            aria-invalid={control['aria-invalid']} data-testid="period" options={[{ value: 'yearly', label: 'Yearly' }]} />}
        </Field>
      </page>;
    }
  `);
  const region = () => screen.queryAll(node => node.props.role === 'status');
  assert.equal(region().length, 1);
  assert.equal(textOf(region()[0]), 'Choose a period.');
  assert.deepEqual(described(screen, screen.getByTestId('period')), ['Choose a period.']);
  assert.equal(screen.getByTestId('period').props.stateInvalid, 1);
  assert.ok(region()[0].children.some(node => node.tag === 'vector' && node.props.accessibilityHidden === 1), 'icon, hidden');
  screen.select(screen.getByTestId('period'), 'yearly');
  assert.equal(region().length, 1, 'still mounted');
  assert.equal(textOf(region()[0]), '');
  assert.equal(region()[0].props.opacity, 0, 'visually hidden while empty');
  assert.equal(screen.getByTestId('period').props.ariaDescribedBy, undefined, 'nothing to describe');
  assert.equal(screen.getByTestId('period').props.stateInvalid, undefined);
});

test('Info: an info button after the label, named by the app, opening the note', async () => {
  const screen = await renderExample('Field/Field', 'Info');
  const button = screen.queryAll(node => node.props.accessibilityLabel === 'About the VAT number')[0];
  assert.equal(button.tag, 'button');
  screen.getByText('optional');
});

test('At large text: the optional mark and the info button stay on the label\'s first line when it wraps', async () => {
  const screen = await renderExample('Field/Field', 'Info');
  const button = screen.queryAll(node => node.tag === 'button' && node.props.accessibilityLabel === 'About the VAT number')[0];
  const row = button.parent;
  assert.deepEqual([row.props.flexDirection, row.props.align], ['row', 'baseline'], 'the label and the optional mark share a baseline, not a centre');
  assert.equal(button.props.alignSelf, 'start');
  assert.deepEqual([button.props.marginTop, button.props.marginBottom, button.props.height], [-7, -7, 32], 'centred on an 18 pt first line and no taller than it');
});
