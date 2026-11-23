import { test } from 'node:test';
import assert from 'node:assert/strict';
import { textOf } from '@lingxia/view/testing';
import { renderExample } from '../testing/harness.mjs';

const rows = screen => screen.queryAll(node => node.props.role === 'row');

test('Invoices: a grid named by its label; every row is a grid of the same tracks, so the columns line up', async () => {
  const screen = await renderExample('Table/Table', 'Invoices');
  const table = screen.getByTestId('invoices');
  assert.equal(table.props.role, 'grid');
  assert.equal(table.props.accessibilityLabel, 'Invoices');
  const [header, body] = table.children;
  assert.equal(header.props.accessibilityHidden, 1, 'each cell is read with its heading');
  assert.deepEqual(header.children.map(textOf), ['Customer', 'Due', 'Amount']);
  assert.equal(rows(screen).length, 4);
  assert.deepEqual(rows(screen).map(row => row.parent), Array(4).fill(body));
  const templates = [header, ...rows(screen)].map(row => row.props.gridTemplateColumns);
  assert.ok(templates[0], 'the header carries a template');
  assert.equal(new Set(templates).size, 1);
  assert.equal(rows(screen)[0].props.minHeight, 44);
});

test('Invoices: a cell is read with its heading and its formatted value; numbers sit at the end edge in tabular figures', async () => {
  const screen = await renderExample('Table/Table', 'Invoices');
  const cells = screen.getByTestId('invoices-INV-1043').children;
  assert.deepEqual(cells.map(cell => [cell.props.role, cell.props.accessibilityLabel]), [['gridcell', 'Customer: Blue Harbor'], ['gridcell', 'Due: 9 Nov'], ['gridcell', 'Amount: $420.50']]);
  assert.ok(cells.every(cell => cell.children[0].props.accessibilityHidden === 1), 'the text is not read a second time');
  const amount = cells[2].children[0];
  assert.deepEqual([amount.text, amount.props.textAlign, amount.props.fontVariantNumeric, amount.props.whiteSpace], ['$420.50', 'end', 'tabular-nums', 'nowrap']);
  const heading = screen.getByTestId('invoices').children[0].children[2];
  assert.equal(heading.props.textAlign, 'end', 'the heading sits over its numbers');
});

test('Pressable: each row is one button read as its cells in order; pressing one selects it', async () => {
  const screen = await renderExample('Table/Table', 'Pressable');
  const row = id => screen.getByTestId(`pressable-${id}`);
  assert.equal(row('INV-1044').tag, 'button');
  assert.equal(row('INV-1044').parent.props.role, 'row');
  assert.equal(row('INV-1044').props.accessibilityLabel, 'Customer: Kestrel & Co, Status: Overdue, Amount: $96.00');
  assert.ok(row('INV-1044').children.every(cell => cell.props.role === undefined), 'the cells are drawn only');
  assert.deepEqual([row('INV-1043').props.stateSelected, row('INV-1044').props.stateSelected], [1, 0]);
  screen.press(row('INV-1044'));
  assert.deepEqual([row('INV-1043').props.stateSelected, row('INV-1044').props.stateSelected], [0, 1]);
  assert.equal(row('INV-1044').props.background, 'accent-soft');
});

test('Scrolling: the rows scroll under a heading that stays outside the scroll; compact rows are 36 px', async () => {
  const screen = await renderExample('Table/Table', 'Scrolling');
  const [header, body] = screen.getByTestId('cities').children;
  assert.equal(body.tag, 'scroll');
  assert.equal(body.props.maxHeight, 216);
  assert.equal(header.props.position, undefined, 'nothing sticky is needed');
  assert.equal(rows(screen).length, 10);
  assert.equal(rows(screen)[0].props.minHeight, 36);
  assert.match(header.props.gridTemplateColumns, /72px$/, 'a width in points is a fixed track');
});

test('StateEmpty: the headings stay and the empty line stands in for the rows', async () => {
  const screen = await renderExample('Table/Table', 'StateEmpty');
  assert.equal(rows(screen).length, 0);
  const [header, empty, measure] = screen.getByTestId('empty').children;
  assert.deepEqual([textOf(header), textOf(empty), measure.props.width, measure.props.height, measure.props.opacity], ['CustomerDueAmount', 'No invoices yet', 0, 0, undefined]);
});

test('Members: where a text column is too narrow the records stack; measured wide, they are columns again', async () => {
  const screen = await renderExample('Table/Table', 'Members');
  const table = () => screen.getByTestId('members');
  // 326 - 32 padding - 36 gaps - 148 fixed leaves 110 pt for 3.5 shares: Role would get 47 pt.
  screen.layout(table(), { width: 326, height: 300 });
  assert.equal(table().children.some(child => textOf(child) === 'NameRoleJoinedHours'), false, 'no heading row');
  const block = screen.getByTestId('members-Ben Okafor');
  assert.equal(block.props.role, 'row');
  assert.equal(block.props.gridTemplateColumns, undefined);
  const [first, pairs] = block.children;
  assert.deepEqual(first.children.map(cell => [cell.props.role, cell.props.accessibilityLabel]), [['gridcell', 'Name: Ben Okafor'], ['gridcell', 'Hours: 918.0']]);
  const [name, hours] = first.children.map(cell => cell.children[0]);
  assert.deepEqual([name.props.fontWeight, hours.props.textAlign, hours.props.fontVariantNumeric], [500, 'end', 'tabular-nums'], 'the name leads; the number keeps the end edge and its figures');
  assert.equal(first.children[1].props.shrink, 0, 'the number is never squeezed');
  assert.deepEqual(pairs.children.map(pair => [pair.props.accessibilityLabel, pair.children.map(part => part.text).join(' ')]), [['Role: Designer', 'Role Designer'], ['Joined: 3 Mar 2023', 'Joined 3 Mar 2023']]);
  assert.deepEqual(pairs.children[0].children.map(part => [part.props.color, part.props.fontSize]), [['text-muted', 13], ['text', 13]]);
  assert.equal(pairs.props.flexWrap, 'wrap', 'a pair moves to the next line whole');

  screen.layout(table(), { width: 700, height: 300 });
  assert.equal(textOf(table().children[0]), 'NameRoleJoinedHours', 'the headings are back');
  assert.ok(screen.getByTestId('members-Ben Okafor').props.gridTemplateColumns);
});

test('layout: columns holds whatever the width; a short text column does not stack a three-column table on a phone', async () => {
  const held = await renderExample('Table/Table', 'MembersColumns');
  held.layout(held.getByTestId('members-columns'), { width: 326, height: 300 });
  assert.ok(held.getByTestId('members-columns-Ben Okafor').props.gridTemplateColumns);
  const three = await renderExample('Table/Table', 'Invoices');
  three.layout(three.getByTestId('invoices'), { width: 326, height: 300 });
  assert.ok(three.getByTestId('invoices-INV-1043').props.gridTemplateColumns, 'Due gets 68 pt and its longest value, "20 Nov", needs about 48');
});

test('At large text: measured wider type stacks the records; a number is never cut and a heading never ends in an ellipsis', async () => {
  const screen = await renderExample('Table/Table', 'Invoices');
  const table = () => screen.getByTestId('invoices');
  const isColumns = () => Boolean(screen.getByTestId('invoices-INV-1043').props.gridTemplateColumns);
  // The measure line is last and unseen: the alphabet in body type, then each number column's widest value.
  const measured = () => table().children.at(-1).children;
  const [alphabet, amount] = measured();
  assert.deepEqual([alphabet.text, amount.text, amount.props.whiteSpace, amount.props.fontVariantNumeric], ['abcdefghijklmnopqrstuvwxyz', '$1,280.00', 'nowrap', 'tabular-nums']);
  const set = (charWidth, amountWidth) => {
    screen.layout(measured()[0], { width: charWidth * 26, height: 22 });
    screen.layout(measured()[1], { width: amountWidth, height: 22 });
  };
  // 326 - 32 padding - 24 gaps leaves 67.5 pt a share: Customer 135, Amount 67.5.
  screen.layout(table(), { width: 326, height: 300 });
  set(8, 64);
  assert.ok(isColumns(), 'at the standard size every column holds its values');
  set(8, 84);
  assert.equal(isColumns(), false, 'a number wider than its column stacks the table rather than run past its edge');
  set(16, 64);
  assert.equal(isColumns(), false, 'at twice the size Customer is under 11 characters: stacked, not broken mid-word');
  const columns = await renderExample('Table/Table', 'MembersColumns');
  const headings = columns.getByTestId('members-columns').children[0].children;
  assert.ok(headings.every(heading => heading.props.numberOfLines === undefined && heading.props.textOverflow === undefined), 'a heading wraps; it is never cut');
});
