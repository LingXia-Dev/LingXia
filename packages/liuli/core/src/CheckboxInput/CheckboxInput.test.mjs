import { test } from 'node:test';
import assert from 'node:assert/strict';
import { textOf } from '@lingxia/view/testing';
import { described, renderExample } from '../testing/harness.mjs';

const mark = node => node.children.length > 0 && JSON.stringify(node).includes('M5 12.5l4.5 4.5L19 7.5');

test('Basic: named by the label, described by the help text; toggling draws and removes the mark', async () => {
  const screen = await renderExample('CheckboxInput/CheckboxInput', 'Basic');
  const box = () => screen.getByTestId('digest');
  assert.equal(box().props.checked, 1);
  assert.equal(box().props.accessibilityLabel, 'Weekly digest');
  assert.deepEqual(described(screen, box()), ['A summary of your notes every Monday.']);
  assert.ok(mark(box()));
  screen.toggle(box(), false);
  assert.equal(box().props.checked, 0);
  assert.ok(!mark(box()));
});

test('Controlled: required and invalid until checked; the error is the description', async () => {
  const screen = await renderExample('CheckboxInput/CheckboxInput', 'Controlled');
  const region = () => screen.queryAll(node => node.props.role === 'status')[0];
  const terms = () => screen.getByTestId('terms');
  assert.equal(textOf(region()), 'Accept the terms to create the account.');
  assert.equal(terms().props.stateRequired, 1);
  assert.equal(terms().props.stateInvalid, 1);
  assert.deepEqual(described(screen, terms()), ['Accept the terms to create the account.']);
  screen.toggle(terms(), true);
  assert.equal(terms().props.checked, 1);
  assert.equal(terms().props.stateInvalid, undefined);
  assert.equal(terms().props.ariaDescribedBy, undefined);
  assert.equal(textOf(region()), '');
});

test('Sizes: three independent boxes', async () => {
  const screen = await renderExample('CheckboxInput/CheckboxInput', 'Sizes');
  assert.deepEqual(screen.queryAll(node => node.tag === 'checkbox').map(node => node.props.checked), [0, 1, 0]);
});
