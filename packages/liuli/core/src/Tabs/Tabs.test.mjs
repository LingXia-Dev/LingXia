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

test('Fitted: tabs keep their labels whole in a row that scrolls; the indicator sits under the chosen tab once the tabs before it are measured', async () => {
  const screen = await renderExample('Tabs/Tabs', 'Fitted');
  const list = () => screen.getByTestId('settings');
  assert.equal(list().parent.tag, 'scroll');
  const tab = value => screen.getByTestId(`settings-${value}`);
  assert.deepEqual([tab('privacy').props.shrink, tab('privacy').children[0].props.whiteSpace, tab('privacy').props.height], [0, 'nowrap', 40]);
  const bar = () => screen.queryAll(node => node.props.background === 'accent' && node.props.height === 2)[0];
  assert.equal(bar(), undefined, 'before any tab is measured there is no indicator');
  const tabs = { general: 74, notifications: 108, privacy: 150, billing: 64, integrations: 102, advanced: 86 };
  for (const [value, width] of Object.entries(tabs)) screen.layout(tab(value).children[1], { width, height: 40 });
  assert.deepEqual([bar().props.width, JSON.parse(bar().props.transform).translate], [74, [0, 0]]);
  screen.press(tab('privacy'));
  assert.equal(tab('privacy').props.stateSelected, 1);
  assert.deepEqual([bar().props.width, JSON.parse(bar().props.transform).translate], [150, [182, 0]], 'after General (74) and Notifications (108)');
});

test('Large text: an equal tab is never narrower than its label; the indicator follows uneven tabs and the row fades where it runs on', async () => {
  const screen = await renderExample('Tabs/Tabs', 'Sections');
  const tab = value => screen.getByTestId(`project-${value}`);
  const bar = () => screen.queryAll(node => node.props.background === 'accent' && node.props.height === 2)[0];
  const scroll = () => screen.getByTestId('project').parent;
  assert.equal(scroll().tag, 'scroll');
  assert.deepEqual([tab('overview').props.grow, tab('overview').props.minWidth, tab('overview').children[0].props.whiteSpace], [1, undefined, 'nowrap'], 'no floor of zero, no cut');
  const frame = scroll().parent;
  screen.layout(frame, { width: 300, height: 44 });
  for (const value of ['overview', 'activity', 'members']) screen.layout(tab(value).children[1], { width: 100, height: 40 });
  assert.equal(Math.round(bar().props.widthPercent), 33, 'even tabs keep the share');
  assert.equal(scroll().props.maskImage, undefined, 'a row that fits does not fade');
  screen.layout(tab('overview').children[1], { width: 130, height: 40 });
  screen.layout(tab('activity').children[1], { width: 120, height: 40 });
  screen.layout(tab('members').children[1], { width: 110, height: 40 });
  assert.deepEqual([bar().props.width, JSON.parse(bar().props.transform).translate], [130, [0, 0]]);
  assert.equal(JSON.parse(scroll().props.maskImage)[0].to, 'left', '360 in 300: the end edge fades');
  const fades = () => JSON.parse(scroll().props.maskImage).map(layer => layer.to);
  screen.fire(scroll(), 'onScrollEdge', JSON.stringify({ edge: 'start', at: false }));
  assert.deepEqual(fades(), ['right', 'left'], 'mid-scroll: both edges fade');
  screen.fire(scroll(), 'onScrollEdge', JSON.stringify({ edge: 'end', at: true }));
  assert.deepEqual(fades(), ['right'], 'at the end: only the start edge fades');
});
