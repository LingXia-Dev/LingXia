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

test('Fitted: tabs keep their labels whole in a row that scrolls; the indicator sits under the chosen tab once the labels before it are measured', async () => {
  const screen = await renderExample('Tabs/Tabs', 'Fitted');
  const list = () => screen.getByTestId('settings');
  assert.equal(list().parent.tag, 'scroll');
  const tab = value => screen.getByTestId(`settings-${value}`);
  assert.deepEqual([tab('privacy').props.shrink, tab('privacy').children[0].props.whiteSpace, tab('privacy').props.height], [0, 'nowrap', 40]);
  const bar = () => screen.queryAll(node => node.props.background === 'accent' && node.props.height === 2)[0];
  assert.equal(bar(), undefined, 'before any label is measured there is no indicator');
  const labels = { general: 50, notifications: 84, privacy: 126, billing: 40, integrations: 78, advanced: 62 };
  for (const [value, width] of Object.entries(labels)) screen.layout(tab(value).children[0], { width, height: 18 });
  assert.deepEqual([bar().props.width, JSON.parse(bar().props.transform).translate], [74, [0, 0]], 'the label and 12 pt each side');
  screen.press(tab('privacy'));
  assert.equal(tab('privacy').props.stateSelected, 1);
  assert.deepEqual([bar().props.width, JSON.parse(bar().props.transform).translate], [150, [182, 0]], 'after General (74) and Notifications (108)');
});
