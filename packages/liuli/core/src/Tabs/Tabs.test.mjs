import { test } from 'node:test';
import assert from 'node:assert/strict';
import { renderExample } from '../testing/harness.mjs';

const indicator = screen => screen.queryAll(node => node.props.widthPercent !== undefined)[0];

test('Sections: one named tab list; the chosen tab is selected and names the panel', async () => {
  const screen = await renderExample('Tabs/Tabs', 'Sections');
  assert.equal(screen.getByTestId('project').props.accessibilityLabel, 'Project');
  const overview = screen.getByTestId('project-overview');
  assert.equal(overview.tag, 'button');
  assert.equal(overview.props.role, 'tab');
  assert.equal(overview.props.stateSelected, 1);
  assert.equal(screen.getByTestId('project-panel').props.accessibilityLabel, 'Overview');
  screen.press(screen.getByTestId('project-activity'));
  assert.equal(screen.getByTestId('project-activity').props.stateSelected, 1);
  assert.equal(screen.getByTestId('project-overview').props.stateSelected, 0);
  assert.equal(screen.getByTestId('project-panel').props.accessibilityLabel, 'Activity');
  screen.getByText('Ana edited "Q3 plan" and two others today.');
});

test('Segmented: the indicator is one tab wide and moves by its own width per tab', async () => {
  const screen = await renderExample('Tabs/Tabs', 'Segmented');
  assert.equal(Math.round(indicator(screen).props.widthPercent), 33);
  assert.deepEqual(JSON.parse(indicator(screen).props.transform).translate, ['100%', 0]);
  screen.press(screen.getByTestId('range-month'));
  assert.deepEqual(JSON.parse(indicator(screen).props.transform).translate, ['200%', 0]);
  screen.getByText('21 hours');
});

test('StateDisabled: a disabled tab stays in the row but cannot be chosen', async () => {
  const screen = await renderExample('Tabs/Tabs', 'StateDisabled');
  const billing = screen.queryAll(node => node.props.elementId?.endsWith('-billing'))[0];
  assert.equal(billing.props.enabled, 0);
  assert.equal(billing.props.onClick, undefined);
});
