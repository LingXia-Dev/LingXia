import { test } from 'node:test';
import assert from 'node:assert/strict';
import { textOf } from '@lingxia/view/testing';
import { renderExample, renderPage } from '../testing/harness.mjs';

/** A category's series columns, between the two margins. */
const columns = slot => slot.children.filter(node => node.props.maxWidth === 36);
/** The four flex shares of a column: space above, bar above zero, bar below zero, space below. */
const shares = column => column.children.filter(part => part.props.flexBasis === 0).map(part => part.props.grow);
const bars = column => column.children.filter(part => part.props.background !== undefined);
/** The zero line's layer: fixed figure lines and the shares above and below zero, top to bottom. */
const zero = screen => screen.queryAll(node => node.props.position === 'absolute' && node.props.insetTop === 0)[0].children
  .map(part => part.props.height ?? part.props.grow);
const page = (props, rows) => `
  import { BarChart } from '@liulidesign/charts/BarChart';
  const rows = ${rows};
  const money = value => '$' + value;
  export function render() { return <page><BarChart data={rows} x="k" y="v" label="Chart" data-testid="chart" ${props} /></page>; }
`;

test('Orders: each weekday is a button named by its day and value; pressing chooses it, reads it out and steps the others back', async () => {
  const screen = await renderExample('BarChart/BarChart', 'Orders');
  const bar = day => screen.getByTestId(`orders-${day}`);
  assert.equal(screen.getByTestId('orders').props.accessibilityLabel, 'Orders by weekday');
  assert.equal(bar('Thu').tag, 'button');
  assert.equal(bar('Thu').props.accessibilityLabel, 'Thu: 163');
  assert.equal(bar('Thu').props.statePressed, 0);
  assert.deepEqual(shares(columns(bar('Thu'))[0]), [24, 163, 0, 0], 'the tallest bar, 187, fills the plot; this one leaves 24 above');
  const fill = day => bars(columns(bar(day))[0])[0].props.background;
  assert.equal(fill('Mon'), 'chart-1/85');
  const readouts = () => screen.queryAll(node => node.props.role === 'status');
  assert.equal(readouts().length, 0, 'every bar carries its value, so there is no readout line and no height kept for one');

  screen.press(bar('Thu'));
  assert.equal(bar('Thu').props.statePressed, 1);
  assert.equal(fill('Thu'), 'chart-1/85');
  assert.equal(fill('Mon'), 'chart-1/30');
  assert.equal(readouts().length, 0);
  screen.press(bar('Thu'));
  assert.equal(bar('Thu').props.statePressed, 0);
  assert.equal(fill('Mon'), 'chart-1/85');
});

test('Orders: seven bars of one series each carry their value over the bar, in tabular figures, unread', async () => {
  const screen = await renderExample('BarChart/BarChart', 'Orders');
  const column = columns(screen.getByTestId('orders-Fri'))[0];
  const [space, figure, up] = column.children;
  assert.equal(space.props.grow, 0);
  assert.equal(figure.props.height, 20);
  assert.equal(figure.props.alignSafe, 1, 'a figure taller than its line grows down over the bar');
  assert.equal(column.props.alignSafe, undefined, 'a figure wider than its bar spills to both sides');
  assert.equal(figure.props.accessibilityHidden, 1, 'the button already reads the value');
  assert.equal(textOf(figure), '187');
  assert.deepEqual([figure.children[0].props.fontVariantNumeric, figure.children[0].props.fontSize, figure.children[0].props.color, figure.children[0].props.whiteSpace], ['tabular-nums', 13, 'text-muted', 'nowrap']);
  assert.deepEqual([up.props.radiusTopLeft, up.props.radiusTopRight, up.props.radiusBottomLeft], [4, 4, undefined]);
  assert.deepEqual(zero(screen), [20, 187, 0], 'the zero line keeps the same figure line as the columns, so it sits at the bars\' feet');
  screen.getByText('7 values from Mon to Sun. Lowest 74 at Sun, highest 187 at Fri. Falling: 132 first, 74 last.');
  const labels = screen.queryAll(node => node.tag === 'text' && ['Mon', 'Sun'].includes(node.text));
  assert.ok(labels.every(label => label.parent.parent.props.accessibilityHidden), 'labels under the bars are not read again');
});

test('a category\'s bars take about two thirds of its width, and the gap inside a group is smaller than the gap between groups', async () => {
  const one = await renderExample('BarChart/BarChart', 'Orders');
  const [before, column, after] = one.getByTestId('orders-Mon').children;
  assert.deepEqual([before.props.grow, column.props.grow, after.props.grow].map(value => Math.round(value * 1000) / 1000), [0.235, 1, 0.235]);
  const grouped = await renderExample('BarChart/BarChart', 'Grouped');
  const slot = grouped.queryAll(node => node.props.accessibilityLabel === 'Q1: Online $42k, In store $31k')[0];
  assert.equal(slot.props.gap, 2, 'two points between the bars of a pair');
  assert.equal(columns(slot).length, 2);
  const margin = slot.children[0].props.grow;
  assert.ok(Math.abs(margin / (2 + 2 * margin) - 0.16) < 0.001, 'each side of a group keeps 16% of the slot, so groups stand 32% apart');
});

test('Grouped: no figures over grouped bars; the top of the scale is a round number above a hairline, and the legend hides a series', async () => {
  const screen = await renderExample('BarChart/BarChart', 'Grouped');
  const plot = () => screen.queryAll(node => node.props.role === 'list' && node.props.accessibilityLabel === 'Sales by quarter and channel')[0];
  const slots = () => plot().children.filter(node => node.props.role === 'listitem');
  assert.deepEqual(slots().map(node => node.props.accessibilityLabel), [
    'Q1: Online $42k, In store $31k', 'Q2: Online $48k, In store $29k', 'Q3: Online $55k, In store $33k', 'Q4: Online $67k, In store $38k',
  ]);
  assert.deepEqual(columns(slots()[0]).map(column => bars(column)[0].props.background), ['chart-1/85', 'chart-2/85']);
  assert.deepEqual(shares(columns(slots()[0])[1]), [49, 31, 0, 0], 'the plot runs to $80k, the round number above $67k');
  const scale = screen.getByText('$80k');
  assert.ok(scale.props.accessibilityHidden);
  assert.equal(scale.props.fontVariantNumeric, 'tabular-nums');
  assert.equal(plot().props.borderTopWidth, 1);
  assert.equal(screen.queryAll(node => node.text === '$42k').length, 0);

  const online = screen.getByTestId('channels-legend-online');
  assert.equal(online.props.statePressed, 1);
  screen.press(online);
  assert.equal(slots()[0].props.accessibilityLabel, 'Q1: In store $31k');
  assert.deepEqual(columns(slots()[0]).map(column => bars(column)[0].props.background), ['chart-2/85'], 'a series keeps its colour when another is hidden');
  assert.equal(textOf(columns(slots()[0])[0].children[1]), '$31k', 'one series of four bars: each bar carries its value again');
  assert.equal(screen.getByTestId('channels-legend-online').props.statePressed, 0);
});

test('Signed: bars below zero hang from the same line, rounded underneath, with their value under them', async () => {
  const screen = await renderExample('BarChart/BarChart', 'Signed');
  const slot = name => screen.queryAll(node => node.props.accessibilityLabel === name)[0];
  const jan = columns(slot('Jan: $12k'))[0];
  const apr = columns(slot('Apr: -$11k'))[0];
  assert.deepEqual(shares(jan), [3, 12, 0, 11]);
  assert.deepEqual(shares(apr), [15, 0, 11, 0]);
  assert.deepEqual(jan.children.map(textOf), ['', '$12k', '', '', '', '']);
  assert.deepEqual(apr.children.map(textOf), ['', '', '', '', '-$11k', '']);
  const down = bars(apr)[1];
  assert.deepEqual([down.props.radiusBottomLeft, down.props.radiusTopLeft], [4, undefined]);
  assert.deepEqual(zero(screen), [20, 15, 11, 20], 'fifteen above zero, eleven below, inside the two figure lines');
});

test('Month: thirty bars are still buttons, packed closer, with the top of the scale and only the first and last day labelled', async () => {
  const screen = await renderExample('BarChart/BarChart', 'Month');
  const buttons = screen.queryAll(node => node.tag === 'button');
  assert.equal(buttons.length, 30);
  assert.equal(buttons[0].children[0].props.grow, 0.125, 'bars take four fifths of a narrow slot');
  assert.equal(screen.getByTestId('steps-14 Jun').props.statePressed, 1);
  const labels = screen.queryAll(node => node.tag === 'text' && / Jun$/.test(node.text ?? '') && node.props.fontSize === 12);
  assert.deepEqual(labels.map(label => label.text), ['1 Jun', '30 Jun']);
  screen.getByText('15000');
  assert.equal(columns(buttons[0])[0].children.length, 4, 'no figure lines in the columns');
});

test('valueLabels: always writes every value, none writes nothing and keeps the bars at full height, and the smallest size is bare', async () => {
  const rows = '[{ k: "a", v: 40 }, { k: "b", v: 187 }, { k: "c", v: 90 }, { k: "d", v: 12 }, { k: "e", v: 60 }, { k: "f", v: 75 }, { k: "g", v: 31 }, { k: "h", v: 5 }, { k: "i", v: 66 }]';
  const figures = screen => screen.queryAll(node => node.tag === 'text' && /^\$/.test(node.text ?? '')).map(node => node.text);
  const auto = await renderPage(page('formatValue={money}', rows));
  assert.deepEqual(figures(auto), ['$200'], 'nine bars: the top of the scale only');
  const always = await renderPage(page('formatValue={money} valueLabels="always"', rows));
  assert.equal(figures(always).length, 9);
  const none = await renderPage(page('formatValue={money} valueLabels="none"', rows));
  assert.deepEqual(figures(none), []);
  assert.deepEqual(zero(none), [187, 0], 'the tallest bar fills the plot');
  const tiny = await renderPage(page('formatValue={money} size="xs"', rows.replace(', { k: "i", v: 66 }', '')));
  assert.deepEqual(figures(tiny), []);
});

test('the top of the scale is the next round number, and a signed chart writes both ends', async () => {
  const many = values => `[${values.map((value, index) => `{ k: "${index}", v: ${value} }`).join(', ')}]`;
  const ends = async values => {
    const screen = await renderPage(page('formatValue={money}', many(values)));
    return screen.queryAll(node => node.tag === 'text' && /^\$/.test(node.text ?? '')).map(node => node.text);
  };
  assert.deepEqual(await ends([1, 2, 3, 4, 5, 6, 7, 8, 67]), ['$80']);
  assert.deepEqual(await ends([1, 2, 3, 4, 5, 6, 7, 8, 108]), ['$120']);
  assert.deepEqual(await ends([0.1, 0.2, 0.1, 0.2, 0.1, 0.2, 0.1, 0.2, 0.31]), ['$0.4']);
  assert.deepEqual(await ends([1, 2, 3, 4, 5, 6, 7, -23, 9000]), ['$10000', '$-25']);
});

test('the readout stays where values are not on the marks: grouped bars, many bars, and a line chart with a chosen row', async () => {
  const many = '[' + Array.from({ length: 9 }, (_, index) => `{ k: "${index}", v: ${index + 1}, w: ${9 - index} }`).join(', ') + ']';
  const readout = screen => screen.queryAll(node => node.props.role === 'status');
  const empty = await renderPage(page('onSelect={() => {}}', many));
  assert.equal(readout(empty).length, 1);
  assert.equal(readout(empty)[0].props.minHeight, 18, 'the line keeps its height, so choosing does not move the plot');
  assert.equal(textOf(readout(empty)[0]), '');
  assert.equal(textOf(readout(await renderPage(page('selectedKey="4" formatValue={money}', many)))[0]), '4$5');
  const grouped = await renderPage(page('selectedKey="1"', many).replace('y="v"', 'y={[{ field: "v", label: "V" }, { field: "w", label: "W" }]}'));
  assert.equal(textOf(readout(grouped)[0]), '1V2W8');
  const always = await renderPage(page('selectedKey="4" valueLabels="always"', many));
  assert.equal(readout(always).length, 0, 'figures forced onto nine bars: no readout');
  const line = await renderExample('LineChart/LineChart', 'Selected');
  assert.equal(textOf(readout(line)[0]), 'AprFree492Team171Pro88');
});

test('past 36 categories the bars are a list, not buttons; rows without a number leave an empty slot', async () => {
  const screen = await renderPage(`
    import { BarChart } from '@liulidesign/charts/BarChart';
    const rows = Array.from({ length: 40 }, (_, index) => ({ n: String(index), v: index === 3 ? null : index }));
    export function render() { return <page><BarChart data={rows} x="n" y="v" label="Forty" onSelect={() => {}} data-testid="forty" /></page>; }
  `);
  assert.equal(screen.queryAll(node => node.tag === 'button').length, 0);
  const items = screen.queryAll(node => node.props.role === 'listitem');
  assert.equal(items.length, 40);
  assert.deepEqual(shares(columns(items[3])[0]), [40, 0, 0, 0]);
});

test('states: a chosen bar set from outside; loading, empty and error replace the plot', async () => {
  const selected = await renderExample('BarChart/BarChart', 'StateSelected');
  assert.equal(selected.queryAll(node => node.props.role === 'status').length, 0);
  assert.equal(selected.queryAll(node => node.props.background === 'chart-1/85').length, 2, 'only the chosen bar, above and below zero, keeps its colour');
  assert.equal((await renderExample('BarChart/BarChart', 'StateLoading')).getByTestId('loading').props.stateBusy, 1);
  (await renderExample('BarChart/BarChart', 'StateEmpty')).getByText('No orders this week');
  const error = await renderExample('BarChart/BarChart', 'StateError');
  assert.equal(error.getByText('Orders could not be loaded').props.role, 'alert');
  assert.equal(error.queryAll(node => node.props.background === 'chart-1/85').length, 0);
});
