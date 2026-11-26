import { test } from 'node:test';
import assert from 'node:assert/strict';
import { textOf } from '@lingxia/view/testing';
import { renderExample, renderPage, isSpinner } from '../testing/harness.mjs';

const results = (screen, id) => screen.getByTestId(`${id}-results`);
const rows = screen => screen.queryAll(node => node.props.role === 'listitem').length;

test('Notes: a search editor named by its label; typing narrows the list and the results line says what is left', async () => {
  const screen = await renderExample('SearchField/SearchField', 'Notes');
  const field = () => screen.getByTestId('notes');
  assert.equal(field().tag, 'input');
  assert.equal(field().props.accessibilityLabel, 'Search notes');
  assert.equal(field().props.enterKeyHint, 'search');
  assert.equal(screen.queryAll(node => node.tag === 'text' && node.text === 'Search notes').length, 0, 'the label is not drawn by default');
  assert.equal(results(screen, 'notes').props.role, 'status');
  assert.equal(textOf(results(screen, 'notes')), '', 'mounted and empty before a search, so the first result is announced');
  assert.equal(rows(screen), 6);

  screen.type(field(), 're');
  assert.equal(rows(screen), 3);
  assert.equal(textOf(results(screen, 'notes')), '3 of 6 notes');
  assert.equal(results(screen, 'notes').props.opacity, undefined, 'the line is drawn');
  screen.type(field(), 'zebra');
  assert.equal(textOf(results(screen, 'notes')), 'No notes match');
  const clear = screen.queryAll(node => node.tag === 'button' && node.props.accessibilityLabel === 'Clear search');
  assert.equal(clear.length, 1);
  screen.press(clear[0]);
  assert.equal(rows(screen), 6);
  assert.equal(screen.queryAll(node => node.tag === 'button' && node.props.accessibilityLabel === 'Clear search').length, 0, 'no clear button without text');
});

test('Loading: busy, with a spinner where the magnifier was', async () => {
  const screen = await renderExample('SearchField/SearchField', 'Loading');
  const field = screen.getByTestId('catalogue');
  const root = screen.queryAll(node => node.props.stateBusy === 1)[0];
  assert.ok(root, 'the field is reported busy');
  assert.equal(screen.queryAll(isSpinner).length, 1);
  assert.equal(textOf(results(screen, 'catalogue')), 'Searching');
  assert.equal(field.props.accessibilityLabel, 'Search the catalogue');
});

test('Labelled and Sizes: the label can be drawn; three heights', async () => {
  const labelled = await renderExample('SearchField/SearchField', 'Labelled');
  assert.equal(labelled.queryAll(node => node.tag === 'text' && node.text === 'Find an order').length, 1);
  const sizes = await renderExample('SearchField/SearchField', 'Sizes');
  const height = id => sizes.getByTestId(id).parent.props.minHeight ?? sizes.getByTestId(id).props.minHeight;
  assert.deepEqual([height('small'), height('large')], [32, 48]);
});

test('states: a result kept for assistive tech stays out of sight; disabled takes no typing', async () => {
  const filled = await renderExample('SearchField/SearchField', 'StateFilled');
  assert.equal(textOf(results(filled, 'filled')), '1 of 6 notes');
  const hidden = await renderPage(`
    import { SearchField } from '@liulidesign/core/SearchField';
    export function render() { return <page><SearchField label="Search" isLabelHidden clearLabel="Clear" value="a" onChange={() => {}} resultsLabel="4 results" isResultsLabelHidden data-testid="quiet" /></page>; }
  `);
  const line = results(hidden, 'quiet');
  assert.deepEqual([textOf(line), line.props.opacity, line.props.role], ['4 results', 0, 'status'], 'read, not drawn');
  const disabled = (await renderExample('SearchField/SearchField', 'StateDisabled')).getByTestId('disabled');
  assert.equal(disabled.props.enabled, 0);
});

test('natural width: in a row the field is as wide as twenty characters of its text plus its edges and magnifier, as a native search input; in a column it fills', async () => {
  const screen = await renderExample('SearchField/SearchField', 'Sizes');
  const struts = screen.queryAll(node => node.tag === 'text' && node.text === '0'.repeat(20));
  const editors = screen.queryAll(node => node.tag === 'input');
  assert.equal(struts.length, editors.length);
  struts.forEach((strut, index) => {
    const { height, opacity, overflow, whiteSpace, paddingInlineStart, paddingInlineEnd, accessibilityHidden, fontSize } = strut.props;
    assert.deepEqual([height, opacity, overflow, whiteSpace, accessibilityHidden], [0, 0, 'hidden', 'nowrap', 1], 'unseen, unread, no taller than nothing');
    assert.deepEqual([paddingInlineStart, paddingInlineEnd], [37, 13], 'border, inset and magnifier before; inset and border after');
    assert.equal(fontSize, editors[index].props.fontSize, 'the characters are the editor\'s, so the width grows with the text');
    assert.deepEqual([strut.parent.props.alignSelf, strut.parent.props.minWidth], ['stretch', 0], 'fills a column; a row may shrink it');
  });
  assert.deepEqual([...new Set(struts.map(strut => strut.props.fontSize))].length, 3, 'three sizes');
});
