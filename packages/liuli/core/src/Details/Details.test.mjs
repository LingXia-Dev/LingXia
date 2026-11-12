import { test } from 'node:test';
import assert from 'node:assert/strict';
import { renderExample } from '../testing/harness.mjs';

test('Single: the summary is a button named by the label that announces its expanded state; the body exists only while open', async () => {
  const screen = await renderExample('Details/Details', 'Single');
  const summary = screen.getByTestId('active-summary');
  assert.equal(summary.tag, 'button');
  assert.equal(summary.props.stateExpanded, 0);
  assert.equal(screen.queryAll(node => node.testId === 'active-body').length, 0);
  screen.press(summary);
  assert.equal(screen.getByTestId('active-summary').props.stateExpanded, 1);
  screen.getByTestId('active-body');
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
