import { test } from 'node:test';
import assert from 'node:assert/strict';
import { textOf } from '@lingxia/view/testing';
import { renderExample, renderPage } from '../testing/harness.mjs';

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
  assert.equal(body.tag, 'list', 'a capped body builds only the rows near the screen');
  assert.equal(body.props.maxHeight, 216);
  assert.equal(header.props.position, undefined, 'nothing sticky is needed');
  assert.ok(rows(screen).length >= 1, 'the rows near the screen are built');
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

const order = (screen, id) => rows(screen).map(row => row.testId ?? row.children[0]?.testId).map(key => key.slice(id.length + 1));

test('Sortable: a heading is a button named by its order; a press sorts ascending, a second descending; the row without an amount stays last', async () => {
  const screen = await renderExample('Table/Table', 'Sortable');
  const heading = field => screen.getByTestId(`sortable-sort-${field}`);
  const header = screen.getByTestId('sortable').children[0];
  assert.notEqual(header.props.accessibilityHidden, 1, 'the headings that sort are reached');
  assert.equal(heading('amount').tag, 'button');
  assert.equal(heading('amount').props.accessibilityLabel, 'Sort by Amount');
  assert.deepEqual(order(screen, 'sortable'), ['INV-1042', 'INV-1043', 'INV-1044', 'INV-1045', 'INV-1046'], 'as given until a heading is pressed');
  screen.press(heading('amount'));
  assert.equal(heading('amount').props.accessibilityLabel, 'Amount, sorted ascending');
  assert.deepEqual(order(screen, 'sortable'), ['INV-1044', 'INV-1043', 'INV-1042', 'INV-1045', 'INV-1046']);
  screen.press(heading('amount'));
  assert.equal(heading('amount').props.accessibilityLabel, 'Amount, sorted descending');
  assert.deepEqual(order(screen, 'sortable'), ['INV-1045', 'INV-1042', 'INV-1043', 'INV-1044', 'INV-1046'], 'empty last both ways');
  screen.press(heading('customer'));
  assert.equal(heading('amount').props.accessibilityLabel, 'Sort by Amount', 'one column orders the rows');
  assert.deepEqual(order(screen, 'sortable'), ['INV-1046', 'INV-1043', 'INV-1044', 'INV-1045', 'INV-1042']);
});

test('SortControlled: the page holds the order; the table sorts by it and asks for the next', async () => {
  const screen = await renderExample('Table/Table', 'SortControlled');
  assert.deepEqual(order(screen, 'sort-controlled'), ['INV-1045', 'INV-1046', 'INV-1043', 'INV-1042', 'INV-1044'], 'latest due first');
  assert.equal(screen.getByTestId('sort-controlled-sort-dueOn').props.accessibilityLabel, 'Due, sorted descending');
  screen.press(screen.getByTestId('sort-controlled-sort-dueOn'));
  assert.deepEqual(order(screen, 'sort-controlled'), ['INV-1044', 'INV-1042', 'INV-1043', 'INV-1046', 'INV-1045']);
});

test('text sorts as the language does: case aside and numbers inside it by value; a column compare wins', async () => {
  const screen = await renderPage(`
    import { Table } from '@liulidesign/core/Table';
    const rows = [{ name: 'item 10', rank: 'b' }, { name: 'Item 9', rank: 'c' }, { name: 'apple', rank: 'a' }];
    export function render() {
      return <page><Table label="Things" data-testid="t" layout="columns" columns={[
        { field: 'name', header: 'Name', isSortable: true },
        { field: 'rank', header: 'Rank', isSortable: true, compare: (a, b) => (a.rank < b.rank ? 1 : a.rank > b.rank ? -1 : 0) },
      ]} rows={rows} keyField="name" /></page>;
    }
  `);
  screen.press(screen.getByTestId('t-sort-name'));
  assert.deepEqual(order(screen, 't'), ['apple', 'Item 9', 'item 10']);
  screen.press(screen.getByTestId('t-sort-rank'));
  assert.deepEqual(order(screen, 't'), ['Item 9', 'item 10', 'apple'], 'compare reversed: c, b, a ascending');
});

test('SortStacked: a menu above the records picks the column; the button beside it turns the order round', async () => {
  const screen = await renderExample('Table/Table', 'SortStacked');
  const trigger = () => screen.getByTestId('sort-stacked-sort');
  assert.equal(trigger().parent.props.accessibilityLabel, 'Sort', 'a group named Sort');
  assert.equal(trigger().props.accessibilityLabel, 'Sort');
  assert.equal(screen.queryAll(node => node.testId === 'sort-stacked-direction').length, 0, 'no order before a column');
  screen.press(trigger());
  screen.press(screen.queryAll(node => node.props.role === 'menuitemradio' && node.props.accessibilityLabel === 'Amount')[0]);
  assert.equal(trigger().props.accessibilityLabel, 'Amount');
  assert.equal(screen.getByTestId('sort-stacked-direction').props.accessibilityLabel, 'Sort descending');
  screen.press(screen.getByTestId('sort-stacked-direction'));
  assert.equal(screen.getByTestId('sort-stacked-direction').props.accessibilityLabel, 'Sort ascending');
  assert.deepEqual(order(screen, 'sort-stacked'), ['INV-1045', 'INV-1042', 'INV-1043', 'INV-1044', 'INV-1046']);
});

test('Selectable: a checkbox on each row, named by its first field, and one for all that is mixed while some are checked', async () => {
  const screen = await renderExample('Table/Table', 'Selectable');
  const box = id => screen.getByTestId(id);
  const status = () => textOf(screen.queryAll(node => node.props.role === 'status')[0]);
  assert.equal(box('selectable-INV-1044-select').props.accessibilityLabel, 'Select Kestrel & Co');
  assert.equal(box('selectable-select-all').props.accessibilityLabel, 'Select all rows');
  assert.equal(box('selectable-select-all').props.mixed, 1);
  assert.equal(status(), '1 selected');
  screen.toggle(box('selectable-INV-1042-select'), true);
  assert.equal(status(), '2 selected');
  assert.equal(box('selectable-INV-1042-select').props.checked, 1);
  screen.toggle(box('selectable-select-all'), true);
  assert.equal(status(), '5 selected');
  assert.equal(box('selectable-select-all').props.checked, 1);
  assert.equal(box('selectable-select-all').props.mixed, undefined);
  screen.toggle(box('selectable-select-all'), false);
  assert.equal(status(), 'None selected');
});

test('SelectableStacked: the checkbox for all leads the bar, beside the sort menu', async () => {
  const screen = await renderExample('Table/Table', 'SelectableStacked');
  const all = () => screen.getByTestId('selectable-stacked-select-all');
  assert.ok(screen.getByTestId('selectable-stacked-sort'));
  assert.equal(all().props.checked, 0);
  screen.toggle(screen.getByTestId('selectable-stacked-INV-1045-select'), true);
  assert.equal(all().props.mixed, 1);
});

test('Stacked at accessibility text sizes: the figure goes under the name, start-aligned', async () => {
  const screen = await renderExample('Table/Table', 'SortStacked', { env: { textScale: 2 } });
  const lead = screen.getByTestId('sort-stacked-INV-1044').children[0];
  assert.notEqual(lead.props.flexDirection, 'row');
  assert.deepEqual(lead.children.map(textOf), ['Kestrel & Co', '$96.00']);
  const small = (await renderExample('Table/Table', 'SortStacked')).getByTestId('sort-stacked-INV-1044').children[0];
  assert.equal(small.props.flexDirection, 'row');
});

test('Loading: the grid is busy, placeholder rows say what loads, and the empty label waits', async () => {
  const screen = await renderExample('Table/Table', 'Loading');
  const grid = screen.getByTestId('loading');
  assert.equal(grid.props.stateBusy, 1);
  const status = screen.queryAll(node => node.props.role === 'status' && node.props.accessibilityLabel === 'Loading Invoices')[0];
  assert.ok(status);
  assert.equal(status.children.length, 4);
  assert.ok(!JSON.stringify(grid).includes('No invoices yet'));
});

test('LoadingStacked: the checkbox for all is off while the rows load', async () => {
  const screen = await renderExample('Table/Table', 'LoadingStacked');
  assert.equal(screen.getByTestId('loading-stacked-select-all').props.enabled, 0);
});

test('LoadingStacked at accessibility text sizes: the figure bar goes under the name bar, as the loaded figure does', async () => {
  const lead = screen => screen.queryAll(node => node.props.accessibilityLabel === 'Loading Invoices')[0].children[0].children[1].children[0];
  const large = lead(await renderExample('Table/Table', 'LoadingStacked', { env: { textScale: 2 } }));
  assert.notEqual(large.props.flexDirection, 'row');
  assert.equal(large.children.length, 2);
  const small = lead(await renderExample('Table/Table', 'LoadingStacked'));
  assert.equal(small.props.flexDirection, 'row');
});

test('a number column named past the 64 characters a box name holds still measures its figures', async () => {
  const field = `amount_${'x'.repeat(70)}`;
  const screen = await renderPage(`
    import { Table } from '@liulidesign/core/Table';
    const rows = [{ id: 'a', ${field}: 1200 }, { id: 'b', ${field}: 7 }];
    export function render() {
      return <page><Table label="Totals" data-testid="t" layout="columns" columns={[
        { field: 'id', header: 'Id' },
        { field: '${field}', header: 'Amount', align: 'end' },
      ]} rows={rows} keyField="id" /></page>;
    }
  `);
  assert.equal(order(screen, 't').length, 2);
});

test('Long: two thousand rows under a cap build only those near the screen', async () => {
  const screen = await renderExample('Table/Table', 'Long');
  const built = rows(screen);
  assert.ok(built.length > 0 && built.length < 100, `${built.length} rows built`);
  assert.equal(screen.getByTestId('orders').children[1].tag, 'list');
});
