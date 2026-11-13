import { test } from 'node:test';
import assert from 'node:assert/strict';
import { textOf } from '@lingxia/view/testing';
import { renderExample } from '../testing/harness.mjs';

test('ViewOptions: a named trigger opens a panel named by its title, with live controls', async () => {
  const screen = await renderExample('Popover/Popover', 'ViewOptions');
  const trigger = screen.getByTestId('options');
  assert.equal(trigger.tag, 'button');
  assert.equal(trigger.props.accessibilityLabel, 'View options');
  assert.equal(trigger.props.stateExpanded, 0);
  assert.equal(screen.queryAll(node => node.testId === 'options-panel').length, 0);
  screen.press(trigger);
  assert.equal(screen.getByTestId('options').props.stateExpanded, 1);
  const panel = screen.getByTestId('options-panel');
  assert.equal(panel.props.accessibilityLabel, 'Show in the list');
  assert.equal(screen.queryAll(node => node.props.role === 'heading').map(textOf).includes('Show in the list'), true);
  screen.toggle(screen.getByTestId('dates'));
  assert.equal(textOf(screen.getByTestId('summary')), 'Previews and Dates');
  assert.equal(screen.queryAll(node => node.testId === 'options-panel').length, 1, 'a control inside leaves it open');
  screen.escape();
  assert.equal(screen.queryAll(node => node.testId === 'options-panel').length, 0);
});

test('Share: opens above the trigger, with the gap on its lower side', async () => {
  const screen = await renderExample('Popover/Popover', 'Share');
  screen.press(screen.getByTestId('share'));
  const panel = screen.getByTestId('share-panel');
  assert.equal(panel.props.marginBottom, 6);
  assert.equal(panel.props.marginTop, undefined);
  screen.press(screen.getByTestId('copy'));
  screen.getByText('Copied');
});
