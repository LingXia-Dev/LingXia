import { test } from 'node:test';
import assert from 'node:assert/strict';
import { textOf } from '@lingxia/view/testing';
import { renderExample } from '../testing/harness.mjs';

test('Address: one column until the form is measured wide enough, then two, with a span taking the whole line', async () => {
  const screen = await renderExample('FormLayout/FormLayout', 'Address');
  const form = () => screen.getByTestId('address');
  const fields = () => form().children[0];
  const span = () => screen.getByTestId('street');
  const spanOf = node => (node.props.gridColumn !== undefined || node.props.gridColumnSpan !== undefined || !node.parent ? node : spanOf(node.parent));
  assert.equal(fields().props.gridTemplateColumns, undefined, 'a column on a phone, and before any measurement');
  screen.layout(form(), { width: 328, height: 600 });
  assert.equal(fields().props.gridTemplateColumns, undefined);
  screen.layout(form(), { width: 640, height: 400 });
  assert.ok(fields().props.gridTemplateColumns, 'two columns from 560 pt');
  assert.ok(JSON.stringify(spanOf(span()).props).includes('2'), 'the street takes both');
  assert.equal(fields().children.length, 7, 'the fields keep their written order');
});

test('Address: the actions follow the fields, at the end edge', async () => {
  const screen = await renderExample('FormLayout/FormLayout', 'Address');
  const actions = screen.getByTestId('address').children[1];
  assert.deepEqual([actions.props.flexDirection, actions.props.justify, actions.props.flexWrap], ['row', 'end', 'wrap']);
  assert.deepEqual(actions.children.map(textOf), ['Cancel', 'Save address']);
});

test('Address and Profile: a form marks one kind of field, never both', async () => {
  const marks = screen => ({ stars: screen.queryAll(node => node.tag === 'span' && textOf(node) === ' *').length, optional: screen.queryAll(node => node.tag === 'text' && node.text === 'optional').length });
  assert.deepEqual(marks(await renderExample('FormLayout/FormLayout', 'Address')), { stars: 0, optional: 1 });
  assert.deepEqual(marks(await renderExample('FormLayout/FormLayout', 'Profile')), { stars: 1, optional: 0 });
});
