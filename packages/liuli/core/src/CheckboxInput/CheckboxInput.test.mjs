import { test } from 'node:test';
import assert from 'node:assert/strict';
import { textOf } from '@lingxia/view/testing';
import { renderExample } from '../testing/harness.mjs';

// The checkbox's change handler is a toggle that takes no value; `screen.toggle(node, checked)`
// sends one and is refused, so these tests call it with the node alone.
const flip = (screen, node) => screen.toggle(node);

const mark = node => node.children.length > 0 && JSON.stringify(node).includes('M5 12.5l4.5 4.5L19 7.5');

test('Basic: label and help text are the control content; toggling draws and removes the mark', async () => {
  const screen = await renderExample('CheckboxInput/CheckboxInput', 'Basic');
  const box = () => screen.getByTestId('digest');
  assert.equal(box().props.checked, 1);
  assert.equal(textOf(box()), 'Weekly digestA summary of your notes every Monday.');
  assert.ok(mark(box()));
  flip(screen, box());
  assert.equal(box().props.checked, 0);
  assert.ok(!mark(box()));
});

test('Controlled: the app decides, and the error clears once checked', async () => {
  const screen = await renderExample('CheckboxInput/CheckboxInput', 'Controlled');
  const region = () => screen.queryAll(node => node.props.role === 'status')[0];
  assert.equal(textOf(region()), 'Accept the terms to create the account.');
  flip(screen, screen.getByTestId('terms'));
  assert.equal(screen.getByTestId('terms').props.checked, 1);
  assert.equal(textOf(region()), '');
});

test('Sizes: three independent boxes', async () => {
  const screen = await renderExample('CheckboxInput/CheckboxInput', 'Sizes');
  assert.deepEqual(screen.queryAll(node => node.tag === 'checkbox').map(node => node.props.checked), [0, 1, 0]);
});
