import { test } from 'node:test';
import assert from 'node:assert/strict';
import { renderExample } from '../testing/harness.mjs';

const button = (screen, id) => screen.getByTestId(id);

test('Phones: the rows are a list named by the field; each remove button names its row', async () => {
  const screen = await renderExample('ListInput/ListInput', 'Phones');
  const list = screen.getByTestId('phones');
  assert.deepEqual([list.props.role, list.props.accessibilityLabel, list.children.length], ['list', 'Phone numbers', 2]);
  assert.equal(button(screen, 'phones-remove-p2').props.accessibilityLabel, 'Remove phone 2');
  assert.equal(screen.getByTestId('phone-1').props.accessibilityLabel, 'Phone 1');
});

test('Phones: add stops at max, remove stops at min, and the row that stays keeps its text', async () => {
  const screen = await renderExample('ListInput/ListInput', 'Phones');
  screen.press(button(screen, 'phones-add'));
  screen.press(button(screen, 'phones-add'));
  assert.equal(screen.getByTestId('phones').children.length, 4);
  assert.equal(button(screen, 'phones-add').props.onClick, undefined, 'four is the most');
  for (const key of ['p2', 'p3', 'p4']) screen.press(button(screen, `phones-remove-${key}`));
  assert.equal(screen.getByTestId('phones').children.length, 1);
  assert.equal(button(screen, 'phones-remove-p1').props.onClick, undefined, 'one row always stays');
  assert.equal(JSON.stringify(screen.getByTestId('phone-1').props).includes('912 345 678'), true, 'the first row kept what was typed');
});

test('Links: with no rows there is no list, only the add button and the error', async () => {
  const screen = await renderExample('ListInput/ListInput', 'Links');
  assert.equal(screen.queryAll(node => node.props.role === 'list').length, 0);
  screen.getByText('Add at least one link.');
  screen.press(button(screen, 'links-add'));
  assert.equal(screen.getByTestId('links').children.length, 1);
});

test('At large text: a row of controls wraps, the kind at a least width and the number under it', async () => {
  const screen = await renderExample('ListInput/ListInput', 'Phones');
  const row = screen.queryAll(node => node.props.flexWrap === 'wrap')[0];
  assert.ok(row, 'the row wraps');
  const kind = row.children[0];
  assert.deepEqual([kind.props.minWidth, kind.props.width], [112, undefined], 'the kind grows with its words from 112 pt');
  assert.equal(row.children[1].props.flexBasis, 128, 'the number wraps under it below 128 pt');
});

test('with no rows the field still carries its test id and data-* props', async () => {
  const { renderPage } = await import('../testing/harness.mjs');
  const screen = await renderPage(`
    import { ListInput } from '@liulidesign/core/ListInput';
    export function render() { return <page><ListInput label="Guests" rowKeys={[]} renderRow={() => null} onAdd={() => {}} onRemove={() => {}} data-testid="guests" data-field="guests" /></page>; }`);
  assert.deepEqual(JSON.parse(screen.getByTestId('guests').props.dataAttributes), { field: 'guests' });
  assert.equal(screen.getByTestId('guests-add').tag, 'button');
});
