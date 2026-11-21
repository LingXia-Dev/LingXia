import { test } from 'node:test';
import assert from 'node:assert/strict';
import { textOf } from '@lingxia/view/testing';
import { renderExample } from '../testing/harness.mjs';

const fine = { env: { pointer: 'fine' } };
const shown = (screen, id) => screen.getByTestId(id).children[1].children.map(node => (node.tag === 'button' ? textOf(node) : '…'));

test('Results, on touch: the arrows around the position, which is announced; the arrows stop at the ends', async () => {
  const screen = await renderExample('Pagination/Pagination', 'Results');
  const control = screen.getByTestId('results');
  assert.deepEqual([control.props.role, control.props.accessibilityLabel], ['group', 'Results pages']);
  const position = () => screen.getByTestId('results').children[1];
  assert.deepEqual([position().text, position().props.role, position().props.fontVariantNumeric], ['Page 3 of 12', 'status', 'tabular-nums']);
  assert.deepEqual(['results-previous', 'results-next'].map(id => screen.getByTestId(id).props.accessibilityLabel), ['Previous page', 'Next page']);
  screen.press(screen.getByTestId('results-next'));
  assert.equal(position().text, 'Page 4 of 12');
  screen.getByText('Showing 61–80 of 240');
  for (let at = 0; at < 3; at++) screen.press(screen.getByTestId('results-previous'));
  assert.equal(position().text, 'Page 1 of 12');
  assert.equal(screen.getByTestId('results-previous').props.onClick, undefined, 'no page before the first');
});

test('Results, with a mouse: page numbers, the current one pressed', async () => {
  const screen = await renderExample('Pagination/Pagination', 'Results', fine);
  assert.deepEqual(shown(screen, 'results'), ['1', '2', '3', '4', '…', '12']);
  assert.equal(screen.getByTestId('results-3').props.statePressed, 1);
  assert.equal(screen.getByTestId('results-4').props.accessibilityLabel, 'Page 4');
});

test('Pages: first, last, the current page and its neighbours; a gap of one page is that page', async () => {
  const screen = await renderExample('Pagination/Pagination', 'Pages');
  assert.deepEqual(shown(screen, 'pages'), ['1', '…', '5', '6', '7', '…', '12']);
  const six = screen.getByTestId('pages-6');
  assert.deepEqual([six.props.background, six.children[0].props.color, six.props.minWidth, six.props.height], ['accent-soft', 'on-accent-soft', 40, 40]);
  screen.press(screen.getByTestId('pages-5'));
  screen.press(screen.getByTestId('pages-previous'));
  assert.deepEqual(shown(screen, 'pages'), ['1', '2', '3', '4', '5', '…', '12'], 'page 2 stands where an ellipsis would hide one page');
  screen.press(screen.getByTestId('pages-12'));
  assert.deepEqual(shown(screen, 'pages'), ['1', '…', '11', '12']);
  assert.equal(screen.getByTestId('pages-next').props.onClick, undefined);
});

test('Compact: the position is "1 / 4" without words from the app', async () => {
  const screen = await renderExample('Pagination/Pagination', 'Compact', fine);
  assert.equal(screen.getByTestId('compact').children[1].text, '1 / 4');
});

test('Pages in a narrow row: fewer neighbours first, then the position', async () => {
  const screen = await renderExample('Pagination/Pagination', 'Pages');
  const control = () => screen.getByTestId('pages');
  screen.layout(control(), { width: 288, height: 40 });
  assert.deepEqual(shown(screen, 'pages'), ['1', '…', '6', '…', '12'], 'seven places need 364 pt; without neighbours, 276');
  screen.layout(control(), { width: 240, height: 40 });
  assert.equal(control().children[1].text, '6 / 12', 'no room for numbers at all');
});

test('Cursor: no count, so the position alone between the arrows; Next stops where nothing follows', async () => {
  const screen = await renderExample('Pagination/Pagination', 'Cursor', fine);
  const position = () => screen.getByTestId('cursor').children[1];
  assert.deepEqual([position().text, position().props.role], ['Page 1', 'status']);
  screen.press(screen.getByTestId('cursor-next'));
  screen.press(screen.getByTestId('cursor-next'));
  assert.equal(position().text, 'Page 3');
  assert.equal(screen.getByTestId('cursor-next').props.onClick, undefined, 'hasMore is false on the last page');
  assert.ok(screen.getByTestId('cursor-previous').props.onClick);
});

test('PageSize: a size choice before the pages; a new size keeps the first row of the page in view', async () => {
  const screen = await renderExample('Pagination/Pagination', 'PageSize', fine);
  const size = screen.getByTestId('sized-size');
  assert.deepEqual([size.tag, size.props.accessibilityLabel, size.props.inputValue], ['select', 'Rows per page', '20']);
  assert.equal(textOf(screen.getByTestId('sized-12')), '12', 'twelve pages of twenty');
  assert.equal(textOf(screen.getByTestId('sized-range')), 'Showing 41–60 of 240');
  screen.select(size, '50');
  assert.equal(textOf(screen.getByTestId('sized-range')), 'Showing 1–50 of 240', 'row 41 is on the first page of fifty');
  assert.equal(textOf(screen.getByTestId('sized-5')), '5', 'five pages of fifty');
  assert.equal(screen.queryAll(node => node.testId === 'sized-6').length, 0);
});
