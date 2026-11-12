import { test } from 'node:test';
import assert from 'node:assert/strict';
import { described, renderExample } from '../testing/harness.mjs';

test('Country: named by its label, described by its help text, and the choice reaches the bound cell', async () => {
  const screen = await renderExample('Select/Select', 'Country');
  const select = screen.getByTestId('country');
  assert.equal(select.tag, 'select');
  assert.equal(select.props.accessibilityLabel, 'Country');
  assert.deepEqual(described(screen, select), ['Sets the currency and the tax on your invoices.']);
  screen.select(select, 'fr');
  assert.equal(screen.getByTestId('country').props.inputValue, 'fr');
});

test('Required: invalid with the error as its description until a role is chosen; the owner option is disabled', async () => {
  const screen = await renderExample('Select/Select', 'Required');
  const select = screen.getByTestId('role');
  assert.equal(select.props.stateInvalid, 1);
  assert.deepEqual(described(screen, select), ['Only an owner can add another owner.', 'Choose a role for the invite.']);
  assert.equal(JSON.parse(select.props.selectOptions).find(option => option.value === 'owner').disabled, true);
  screen.select(select, 'editor');
  assert.equal(screen.getByTestId('role').props.stateInvalid, undefined);
});
