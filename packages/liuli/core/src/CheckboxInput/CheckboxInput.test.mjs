import { test } from 'node:test';
import assert from 'node:assert/strict';
import { textOf } from '@lingxia/view/testing';
import { described, renderExample } from '../testing/harness.mjs';

const mark = node => node.children.length > 0 && JSON.stringify(node).includes('M20 6 9 17l-5-5');

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

test('the status message starts under the label, past the box and the row gap', async () => {
  const screen = await renderExample('CheckboxInput/CheckboxInput', 'Controlled');
  const region = screen.queryAll(node => node.props.role === 'status')[0];
  const row = screen.getByTestId('terms').children[0];
  const box = row.children[0];
  assert.equal(region.props.paddingLeft ?? region.props.paddingInlineStart, box.props.width + row.props.gap);
  const control = screen.getByTestId('terms').props;
  const margin = control.marginLeft ?? control.marginInlineStart;
  const padding = control.paddingLeft ?? control.paddingInlineStart;
  assert.ok(padding > 0, 'the focus fill reaches past the box');
  assert.equal(margin + padding, 0, 'no net inset before the box');
});

test('StateMixed: a dash on the filled box, read as mixed', async () => {
  const box = (await renderExample('CheckboxInput/CheckboxInput', 'StateMixed')).getByTestId('mixed');
  assert.deepEqual([box.props.mixed, JSON.parse(box.props.dataAttributes).state], [1, 'mixed']);
});

test('At large text: the box keeps its size and shape, never squeezed by its label', async () => {
  const screen = await renderExample('CheckboxInput/CheckboxInput', 'Sizes');
  const boxes = screen.queryAll(node => node.props.borderTopWidth === 2 && node.props.accessibilityHidden === 1);
  assert.equal(boxes.length, 3);
  for (const box of boxes) {
    const { shrink, width, height } = box.props;
    assert.equal(shrink, 0);
    assert.ok(width > 0 && width === height);
  }
});
