import { test } from 'node:test';
import assert from 'node:assert/strict';
import { textOf } from '@lingxia/view/testing';
import { renderExample } from '../testing/harness.mjs';

const sheet = (screen, testId) => screen.queryAll(node => node.testId === testId)[0];
const phone = { env: { sizeClass: 'compact', viewport: { width: 393, height: 852 } } };
const wide = { env: { sizeClass: 'regular', viewport: { width: 1024, height: 768 } } };

test('Basic on a phone: a named dialog on the bottom edge that drags off; a row closes it and runs its action', async () => {
  const screen = await renderExample('ActionSheet/ActionSheet', 'Basic', phone);
  assert.equal(sheet(screen, 'photo'), undefined);
  screen.press(screen.getByTestId('photo-open'));
  const panel = sheet(screen, 'photo');
  assert.equal(panel.props.accessibilityLabel, 'This photo');
  assert.deepEqual(JSON.parse(panel.props.drag), { axis: 'y', points: [0, '100%'], index: 0 });
  screen.press(screen.getByTestId('photo-duplicate'));
  assert.equal(sheet(screen, 'photo'), undefined);
  assert.equal(textOf(screen.getByTestId('photo-done')), 'Duplicated');
});

test('Basic on a wide window: a panel in the middle that does not drag', async () => {
  const screen = await renderExample('ActionSheet/ActionSheet', 'Basic', wide);
  screen.press(screen.getByTestId('photo-open'));
  assert.equal(sheet(screen, 'photo').props.drag, undefined);
});

test('Basic: Cancel, Escape and a drag off all close it without running anything', async () => {
  const screen = await renderExample('ActionSheet/ActionSheet', 'Basic', phone);
  screen.press(screen.getByTestId('photo-open'));
  screen.press(screen.getByTestId('photo-cancel'));
  assert.equal(sheet(screen, 'photo'), undefined);
  screen.press(screen.getByTestId('photo-open'));
  assert.equal(screen.escape(), true);
  assert.equal(sheet(screen, 'photo'), undefined);
  screen.press(screen.getByTestId('photo-open'));
  screen.drag(sheet(screen, 'photo'), { phase: 'settle', index: 1, hidden: true });
  assert.equal(sheet(screen, 'photo'), undefined);
  assert.equal(textOf(screen.getByTestId('photo-done')), 'Nothing yet');
});

test('Basic: the destructive row is in the danger colour, the others in the text colour', async () => {
  const screen = await renderExample('ActionSheet/ActionSheet', 'Basic', phone);
  screen.press(screen.getByTestId('photo-open'));
  assert.equal(screen.getByTestId('photo-delete').props.color, 'danger');
  assert.equal(screen.getByTestId('photo-share').props.color, 'text');
  assert.equal(screen.getByTestId('photo-cancel').props.color, 'text');
});

test('WithMessage: the message is the dialog\'s description; Untitled keeps its title as its name only', async () => {
  const screen = await renderExample('ActionSheet/ActionSheet', 'WithMessage', phone);
  screen.press(screen.getByTestId('draft-open'));
  const panel = sheet(screen, 'draft');
  const described = screen.queryAll(node => node.props.elementId !== undefined && node.props.elementId === panel.props.ariaDescribedBy);
  assert.equal(textOf(described[0]), 'Unsaved changes are lost if you discard it.');
  const untitled = await renderExample('ActionSheet/ActionSheet', 'Untitled', phone);
  untitled.press(untitled.getByTestId('order-open'));
  assert.equal(sheet(untitled, 'order').props.accessibilityLabel, 'Order');
  assert.equal(untitled.queryAll(node => node.props.role === 'heading').length, 0);
});
