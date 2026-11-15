import { test } from 'node:test';
import assert from 'node:assert/strict';
import { renderExample } from '../testing/harness.mjs';

test('Single: the summary is a button named by the label that announces its expanded state; the closed body is clipped to nothing and hidden', async () => {
  const screen = await renderExample('Details/Details', 'Single');
  const summary = screen.getByTestId('active-summary');
  assert.equal(summary.tag, 'button');
  assert.equal(summary.props.stateExpanded, 0);
  const body = () => screen.getByTestId('active-body');
  assert.deepEqual([body().props.height, body().props.overflow, body().props.accessibilityHidden], [0, 'hidden', 1]);
  screen.press(summary);
  assert.equal(screen.getByTestId('active-summary').props.stateExpanded, 1);
  assert.deepEqual([body().props.height, body().props.accessibilityHidden], [undefined, undefined], 'open: its own height, and read');
  assert.ok(String(body().props.transition).includes('height') || String(body().props.transition).includes('size'), 'the box animates its height');
});

test('Accordion: opening one row closes the open one; closing it leaves none open', async () => {
  const screen = await renderExample('Details/Details', 'Accordion');
  const expanded = id => screen.getByTestId(`${id}-summary`).props.stateExpanded;
  assert.equal(expanded('refunds'), 1);
  screen.press(screen.getByTestId('export-summary'));
  assert.equal(expanded('export'), 1);
  assert.equal(expanded('refunds'), 0);
  screen.press(screen.getByTestId('export-summary'));
  assert.equal(expanded('export'), 0);
});

test('Independent: rows open and close on their own', async () => {
  const screen = await renderExample('Details/Details', 'Independent');
  assert.equal(screen.queryAll(node => node.tag === 'button' && node.props.stateExpanded === 1).length, 2);
});

test('Sections: a named group the app controls; a row carries an icon and a second line; a disabled row does not open', async () => {
  const screen = await renderExample('Details/Details', 'Sections');
  const group = screen.getByTestId('sections');
  assert.deepEqual([group.props.role, group.props.accessibilityLabel], ['group', 'Settings']);
  const summary = id => screen.getByTestId(`section-${id}-summary`);
  assert.equal(summary('notifications').props.stateExpanded, 1);
  assert.equal(summary('privacy').children[0].tag, 'vector');
  assert.equal(summary('privacy').children[0].props.accessibilityHidden, 1);
  assert.deepEqual(summary('privacy').children[1].children.map(node => node.text), ['Privacy', 'Who can see your profile']);
  screen.press(summary('privacy'));
  assert.deepEqual([summary('notifications').props.stateExpanded, summary('privacy').props.stateExpanded], [0, 1]);
  screen.press(summary('privacy'));
  assert.equal(summary('privacy').props.stateExpanded, 0, 'the open row closes, and none is open');
  assert.equal(summary('billing').props.onClick, undefined);
});

test('WithForm: what is typed inside a row is kept while the row is closed', async () => {
  const screen = await renderExample('Details/Details', 'WithForm');
  screen.press(screen.getByTestId('form-summary'));
  screen.type(screen.getByTestId('form-name'), 'Kai');
  screen.press(screen.getByTestId('form-summary'));
  assert.equal(screen.getByTestId('form-body').props.accessibilityHidden, 1);
  assert.equal(screen.getByTestId('form-name').props.inputValue, 'Kai');
});
