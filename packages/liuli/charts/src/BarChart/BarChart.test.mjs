import { test } from 'node:test';
import assert from 'node:assert/strict';
import { textOf } from '@lingxia/view/testing';
import { renderExample, renderPage } from '../testing/harness.mjs';

/** A chart's drawings, in order: bars above zero, those stepped back, bars below zero, those stepped back. */
const drawings = chart => find(chart, node => node.tag === 'vector' && node.props.grow === 1 && node.props.opacity !== undefined)
  .map(node => ({ ...JSON.parse(node.props.vectorSource), opacity: node.props.opacity, wrap: node.parent.props, region: node.parent.parent.props }));
const find = (node, test) => (test(node) ? [node] : node.children.flatMap(child => find(child, test)));
/** Each bar outline of a path as [x, top or bottom, width]. */
const bars = d => [...d.matchAll(/M([\d.]+) (?:100|0)V([\d.]+)h([\d.]+)/g)].map(match => match.slice(1).map(Number));
const slots = chart => find(chart, node => (node.tag === 'button' || node.props.role === 'listitem') && /: /.test(node.props.accessibilityLabel ?? ''));
/** A slot's figure line: [share above, the figure, share below]. */
const figure = slot => slot.children.map(part => part.props.grow ?? textOf(part));
const count = node => 1 + node.children.reduce((sum, child) => sum + count(child), 0);
const page = (props, rows) => `
  import { BarChart } from '@liulidesign/charts/BarChart';
  const rows = ${rows};
  const money = value => '$' + value;
  export function render() { return <page><BarChart data={rows} x="k" y="v" label="Chart" data-testid="chart" ${props} /></page>; }
`;
const many = (length, value = 'index + 1') => `Array.from({ length: ${length} }, (_, index) => ({ k: String(index), v: ${value}, w: ${length} - index }))`;

test('Orders: all seven bars are one outline in one drawing; each bar runs from its value to the foot', async () => {
  const screen = await renderExample('BarChart/BarChart', 'Orders');
  const chart = screen.getByTestId('orders');
  const [up, ...rest] = drawings(chart);
  assert.equal(rest.length, 0, 'nothing below zero and nothing stepped back');
  assert.equal(up.paths.length, 1);
  assert.equal(up.fit, 'fill');
  const outline = up.paths[0];
  assert.deepEqual([outline.fill, outline.stroke, outline.strokeWidth, outline.strokeLinejoin, outline.vectorEffect], ['chart-1', 'chart-1', 6, 'round', 'non-scaling-stroke']);
  assert.equal(up.opacity, 0.85, 'the series colour a little under full strength');
  const drawn = bars(outline.d);
  assert.equal(drawn.length, 7);
  assert.deepEqual(drawn[4], [60.71, 0, 7.14], 'Friday, the highest, reaches the top; a bar is half its slot wide');
  assert.deepEqual(drawn[3].slice(0, 2), [46.43, 12.83], 'Thursday: 163 of 187');
  // The drawing is 3 pt taller than its region at the zero line and starts 3 pt down, so the
  // stroke that rounds each corner ends exactly at the value and is hidden at the foot.
  assert.deepEqual([up.wrap.insetTop, up.wrap.insetBottom, up.wrap.paddingTop], [0, -3, 3]);
  assert.equal(up.region.overflow, 'hidden');
});

test('Orders: each weekday is a button named by its day and value, carrying its figure level with its bar', async () => {
  const screen = await renderExample('BarChart/BarChart', 'Orders');
  const bar = day => screen.getByTestId(`orders-${day}`);
  assert.equal(bar('Thu').tag, 'button');
  assert.equal(bar('Thu').props.accessibilityLabel, 'Thu: 163');
  assert.equal(bar('Thu').props.statePressed, 0);
  assert.deepEqual(figure(bar('Thu')), [24, '163', 163], 'twenty-four of 187 above the figure, 163 under it');
  const line = bar('Thu').children[1];
  assert.deepEqual([line.props.height, line.props.accessibilityHidden, line.props.alignSafe], [20, 1, 1], 'the button already reads the value');
  assert.deepEqual([line.children[0].props.fontVariantNumeric, line.children[0].props.fontSize, line.children[0].props.color, line.children[0].props.whiteSpace], ['tabular-nums', 13, 'text-muted', 'nowrap']);
  const marks = find(screen.getByTestId('orders'), node => node.props.position === 'absolute' && node.props.insetTop === 20)[0];
  assert.ok(marks, 'the drawing starts under the figures\' line, so a figure sits on its bar');
  assert.equal(screen.queryAll(node => node.props.role === 'status').length, 0, 'every bar carries its value, so there is no readout line');
  screen.getByText('7 values from Mon to Sun. Lowest 74 at Sun, highest 187 at Fri. Falling: 132 first, 74 last.');
  const labels = screen.queryAll(node => node.tag === 'text' && ['Mon', 'Sun'].includes(node.text));
  const unread = node => Boolean(node) && (Boolean(node.props.accessibilityHidden) || unread(node.parent));
  assert.ok(labels.length >= 2 && labels.every(unread), 'labels under the bars are not read again');
});

test('Orders: pressing a bar chooses it; the others move to a second, fainter drawing', async () => {
  const screen = await renderExample('BarChart/BarChart', 'Orders');
  const bar = day => screen.getByTestId(`orders-${day}`);
  screen.press(bar('Thu'));
  assert.equal(bar('Thu').props.statePressed, 1);
  const [chosen, others] = drawings(screen.getByTestId('orders'));
  assert.deepEqual([bars(chosen.paths[0].d).length, chosen.opacity], [1, 0.85]);
  assert.deepEqual([bars(others.paths[0].d).length, others.opacity], [6, 0.3]);
  assert.equal(bars(chosen.paths[0].d)[0][0], 46.43, 'Thursday keeps its place');
  screen.press(bar('Thu'));
  assert.equal(bar('Thu').props.statePressed, 0);
  assert.equal(drawings(screen.getByTestId('orders')).length, 1);
});

test('Grouped: one outline per series, a pair closer than two groups; the scale\'s top is a round number; the legend hides a series', async () => {
  const screen = await renderExample('BarChart/BarChart', 'Grouped');
  const chart = () => screen.getByTestId('channels');
  assert.deepEqual(slots(chart()).map(node => node.props.accessibilityLabel), [
    'Q1: Online $42k, In store $31k', 'Q2: Online $48k, In store $29k', 'Q3: Online $55k, In store $33k', 'Q4: Online $67k, In store $38k',
  ]);
  const [up] = drawings(chart());
  assert.deepEqual(up.paths.map(path => path.fill), ['chart-1', 'chart-2']);
  const [online, store] = up.paths.map(path => bars(path.d));
  assert.deepEqual(online[0], [4.75, 47.5, 6.5], '$42k of a plot that runs to $80k, the round number above $67k');
  const pairGap = store[0][0] - (online[0][0] + online[0][2]);
  const groupGap = online[1][0] - (store[0][0] + store[0][2]);
  assert.ok(Math.abs(pairGap - 2.5) < 0.01 && Math.abs(groupGap - 9.5) < 0.01, `pair ${pairGap}, groups ${groupGap}`);
  const scale = screen.getByText('$80k');
  assert.ok(scale.props.accessibilityHidden);
  assert.equal(scale.props.fontVariantNumeric, 'tabular-nums');
  assert.equal(scale.parent.children.find(node => node.props.position === 'relative').props.borderTopWidth, 1);
  assert.equal(screen.queryAll(node => node.text === '$42k').length, 0, 'no figures over grouped bars');
  assert.ok(slots(chart()).every(slot => slot.children.length === 0));

  const legend = screen.getByTestId('channels-legend-online');
  assert.equal(legend.props.statePressed, 1);
  screen.press(legend);
  assert.equal(slots(chart())[0].props.accessibilityLabel, 'Q1: In store $31k');
  assert.deepEqual(drawings(chart())[0].paths.map(path => path.fill), ['chart-2'], 'a series keeps its colour when another is hidden');
  assert.deepEqual(figure(slots(chart())[0]), [7, '$31k', 31], 'one series of four bars: each bar carries its value, and the range refits');
  assert.equal(screen.getByTestId('channels-legend-online').props.statePressed, 0);
});

test('Signed: bars below zero hang from the same line in their own drawing, with their value under them', async () => {
  const screen = await renderExample('BarChart/BarChart', 'Signed');
  const chart = screen.getByTestId('cash');
  const [up, down] = drawings(chart);
  assert.deepEqual([up.region.grow, down.region.grow], [15, 11], 'fifteen above zero, eleven below');
  assert.deepEqual(bars(up.paths[0].d).map(bar => bar[1]), [20, 46.67, 80, 0], '12, 8, 3 and 15 of 15');
  assert.deepEqual(bars(down.paths[0].d).map(bar => bar[1]), [45.45, 100], '5 and 11 of 11, measured down from the zero line');
  assert.deepEqual([down.wrap.insetBottom, down.wrap.insetTop, down.wrap.paddingBottom], [0, -3, 3]);
  const zero = up.region && find(chart, node => node.props.height === 1 && node.props.background === 'text/20');
  assert.equal(zero.length, 1);
  const slot = name => slots(chart).find(node => node.props.accessibilityLabel === name);
  assert.deepEqual([figure(slot('Jan: $12k')), slot('Jan: $12k').props.paddingBottom], [[3, '$12k', 23], 20]);
  assert.deepEqual([figure(slot('Apr: -$11k')), slot('Apr: -$11k').props.paddingTop], [[26, '-$11k', 0], 20]);
  const marks = find(chart, node => node.props.position === 'absolute' && node.props.insetTop === 20)[0];
  assert.equal(marks.props.insetBottom, 20, 'a figures\' line is kept on both sides');
});

test('Month: thirty bars are still buttons, with a smaller corner, the top of the scale and, until the row is measured, the first and last day labelled', async () => {
  const screen = await renderExample('BarChart/BarChart', 'Month');
  const chart = screen.getByTestId('steps');
  const buttons = slots(chart);
  assert.equal(buttons.length, 30);
  assert.ok(buttons.every(button => button.tag === 'button' && button.children.length === 0));
  assert.equal(screen.getByTestId('steps-14 Jun').props.statePressed, 1);
  const [chosen, others] = drawings(chart);
  assert.deepEqual([bars(chosen.paths[0].d).length, bars(others.paths[0].d).length], [1, 29]);
  assert.deepEqual([chosen.paths[0].strokeWidth, chosen.wrap.insetBottom, chosen.wrap.paddingTop], [2, -1, 1]);
  const labels = screen.queryAll(node => node.tag === 'text' && / Jun$/.test(node.text ?? '') && node.props.fontSize === 12 && node.props.opacity !== 0);
  assert.deepEqual(labels.map(label => label.text), ['1 Jun', '30 Jun']);
  screen.getByText('15000');
});

test('a chart costs one element per bar and a fixed handful more, however many bars it has', async () => {
  const sized = async (length, props = '') => count((await renderPage(page(props, many(length)))).getByTestId('chart'));
  const [ten, sixty] = [await sized(10), await sized(60)];
  assert.equal(sixty - ten, 50, 'fifty more bars, fifty more elements');
  assert.ok(sixty <= 60 + 19, `${sixty} elements for 60 bars`);
  assert.ok(await sized(60, 'onSelect={() => {}} selectedKey="7"') <= 60 + 23, 'a choice adds one drawing and the readout');
  const eight = await sized(8);
  assert.ok(eight <= 8 * 5 + 17, `${eight} elements for 8 bars with their figures`);
  const grouped = count((await renderPage(page('', many(60)).replace('y="v"', 'y={[{ field: "v", label: "V" }, { field: "w", label: "W" }]}'))).getByTestId('chart'));
  assert.ok(grouped <= 60 + 29, `${grouped} elements for 60 groups of two with a legend`);
});

test('a theme change or an unrelated render draws the same outlines', async () => {
  const screen = await renderPage(`
    import { useLocal } from '@lingxia/view';
    import { BarChart } from '@liulidesign/charts/BarChart';
    const rows = ${many(12)};
    export function render() {
      const tick = useLocal('tick', 0);
      return <page><BarChart data={rows} x="k" y="v" label="Chart" data-testid="chart" /><button data-testid="again" onClick={() => tick.set(tick.value + 1)}>{String(tick.value)}</button></page>;
    }
  `);
  const source = () => find(screen.getByTestId('chart'), node => node.tag === 'vector')[0].props.vectorSource;
  const before = source();
  assert.doesNotMatch(before, /#[0-9a-f]{3,8}/i, 'colours are role names, resolved by the theme outside the page');
  screen.press(screen.getByTestId('again'));
  screen.rerender({ env: { theme: 'dark' } });
  assert.equal(source(), before);
});

test('valueLabels: always writes every value, none writes nothing and keeps the bars at full height, and the smallest size is bare', async () => {
  const rows = many(9, '[40, 187, 90, 12, 60, 75, 31, 5, 66][index]');
  const figures = screen => screen.queryAll(node => node.tag === 'text' && /^\$/.test(node.text ?? '')).map(node => node.text);
  const auto = await renderPage(page('formatValue={money}', rows));
  assert.deepEqual(figures(auto), ['$200'], 'nine bars: the top of the scale only');
  const always = await renderPage(page('formatValue={money} valueLabels="always"', rows));
  assert.equal(figures(always).length, 9);
  const none = await renderPage(page('formatValue={money} valueLabels="none"', rows));
  assert.deepEqual(figures(none), []);
  assert.equal(bars(drawings(none.getByTestId('chart'))[0].paths[0].d)[1][1], 0, 'the tallest bar fills the plot');
  const tiny = await renderPage(page('formatValue={money} size="xs"', many(8)));
  assert.deepEqual(figures(tiny), []);
});

test('the top of the scale is the next round number, and a signed chart writes both ends', async () => {
  const ends = async values => {
    const screen = await renderPage(page('formatValue={money}', `[${values.map((value, index) => `{ k: "${index}", v: ${value} }`).join(', ')}]`));
    return screen.queryAll(node => node.tag === 'text' && /^\$/.test(node.text ?? '')).map(node => node.text);
  };
  assert.deepEqual(await ends([1, 2, 3, 4, 5, 6, 7, 8, 67]), ['$80']);
  assert.deepEqual(await ends([1, 2, 3, 4, 5, 6, 7, 8, 108]), ['$120']);
  assert.deepEqual(await ends([0.1, 0.2, 0.1, 0.2, 0.1, 0.2, 0.1, 0.2, 0.31]), ['$0.4']);
  assert.deepEqual(await ends([1, 2, 3, 4, 5, 6, 7, -23, 9000]), ['$10000', '$-25']);
});

test('the readout stays where values are not on the marks: grouped bars, many bars, and a line chart with a chosen row', async () => {
  const readout = screen => screen.queryAll(node => node.props.role === 'status');
  const empty = await renderPage(page('onSelect={() => {}}', many(9)));
  assert.equal(readout(empty).length, 1);
  assert.equal(readout(empty)[0].props.minHeight, 18, 'the line keeps its height, so choosing does not move the plot');
  assert.equal(textOf(readout(empty)[0]), '');
  assert.equal(textOf(readout(await renderPage(page('selectedKey="4" formatValue={money}', many(9))))[0]), '4$5');
  const grouped = await renderPage(page('selectedKey="1"', many(9)).replace('y="v"', 'y={[{ field: "v", label: "V" }, { field: "w", label: "W" }]}'));
  assert.equal(textOf(readout(grouped)[0]), '1V2W8');
  assert.equal(readout(await renderPage(page('selectedKey="4" valueLabels="always"', many(9)))).length, 0, 'figures forced onto nine bars: no readout');
  assert.equal(textOf(readout(await renderExample('LineChart/LineChart', 'Selected'))[0]), 'AprFree492Team171Pro88');
});

test('Orders: a drag across the bars chooses each it passes; a press that stays on its bar is left to the button', async () => {
  const screen = await renderExample('BarChart/BarChart', 'Orders');
  const plot = () => screen.getByTestId('orders-plot');
  const pressed = () => slots(screen.getByTestId('orders')).filter(slot => slot.props.statePressed === 1).map(slot => slot.props.accessibilityLabel);
  const touch = (phase, x) => screen.pointer(plot(), { phase, x, y: 60, width: 280, height: 176 });
  assert.equal(plot().props.pointerAxis, 'x', 'vertical movement stays with the page');
  assert.equal(plot().props.ariaValueNow, undefined, 'the buttons are the keyboard path; the plot is no extra stop');
  touch('down', 20);
  touch('move', 30);
  touch('up', 30);
  assert.deepEqual(pressed(), [], 'landing, a small move inside the bar and lifting choose nothing: the button\'s click does');
  touch('down', 20);
  touch('move', 100);
  assert.deepEqual(pressed(), ['Wed: 121'], 'seven slots of 40 pt');
  touch('move', 900);
  assert.deepEqual(pressed(), ['Sun: 74'], 'past the edge it stays on the last bar');
  touch('move', 20);
  assert.deepEqual(pressed(), ['Mon: 132'], 'back on the bar it started on, once it has left it');
  touch('up', 20);
  touch('down', 20);
  screen.pointer(plot(), { phase: 'move', x: 62, y: 140, width: 280, height: 176 });
  assert.deepEqual(pressed(), ['Mon: 132'], 'a drag that runs more down than across is the page\'s scroll');
  touch('move', 150);
  assert.deepEqual(pressed(), ['Thu: 163']);
  touch('cancel', 150);
  assert.deepEqual(pressed(), ['Mon: 132'], 'a press the page takes after all puts the choice back');
  screen.pointer(plot(), { phase: 'hover', x: 150, y: 60, width: 280, height: 176, pointer: 'mouse' });
  assert.deepEqual(pressed(), ['Mon: 132'], 'a mouse passing over chooses nothing: a click does');
});

test('past 36 categories the plot is one stop that a drag, a tap, the arrow keys and a screen reader move through', async () => {
  const source = page('onSelect={day.set} selectedKey={day.value}', many(40))
    .replace("import { BarChart }", "import { useLocal } from '@lingxia/view';\n  import { BarChart }")
    .replace('export function render() {', 'export function render() { const day = useLocal("day", null);');
  const screen = await renderPage(source);
  const plot = () => screen.getByTestId('chart-plot');
  const readout = () => textOf(screen.queryAll(node => node.props.role === 'status')[0]);
  assert.deepEqual([plot().props.ariaValueNow, plot().props.ariaValueMin, plot().props.ariaValueMax, plot().props.ariaValueText], [39, 0, 39, '39: 40'], 'it rests on the last category');
  const step = phase => screen.pointer(plot(), { phase, x: 0, y: 0, width: 0, height: 0, pointer: 'keyboard' });
  step('decrement');
  assert.equal(readout(), '3839');
  assert.equal(plot().props.ariaValueText, '38: 39');
  const touch = (phase, x) => screen.pointer(plot(), { phase, x, y: 60, width: 320, height: 176 });
  touch('down', 4);
  assert.equal(readout(), '3839', 'landing chooses nothing');
  touch('up', 4);
  assert.equal(readout(), '01', 'a tap chooses the bar it lifts on');
  touch('down', 4);
  touch('move', 84);
  assert.equal(readout(), '1011');
});

test('category labels: every label once the row shows they fit, each under its bar; otherwise every nth from the first', async () => {
  const labels = screen => screen.queryAll(node => node.tag === 'text' && node.props.fontSize === 12 && node.props.opacity !== 0);
  const measure = (screen, width, tick) => {
    const ghost = () => screen.queryAll(node => node.tag === 'text' && node.props.fontSize === 12 && node.props.opacity === 0)[0];
    screen.layout(ghost().parent, { width, height: 16 });
    screen.layout(ghost(), { width: tick, height: 16 });
  };
  const ten = await renderPage(page('', many(10)));
  assert.deepEqual(labels(ten).map(node => node.text), ['0', '9'], 'before the row is measured, past seven bars: the ends');
  measure(ten, 300, 10);
  assert.deepEqual(labels(ten).map(node => node.text), ['0', '1', '2', '3', '4', '5', '6', '7', '8', '9']);
  assert.deepEqual(labels(ten).map(node => node.props.insetLeft), [10, 40, 70, 100, 130, 160, 190, 220, 250, 280], 'centred in slots of 30 pt');
  measure(ten, 300, 40);
  assert.deepEqual(labels(ten).map(node => node.text), ['0', '3', '6', '9'], '48 pt a label and 5 more for the one an edge holds in, 30 pt a slot: every third lands on the last');
  const all = await renderPage(page('xLabels="all"', many(10)));
  assert.equal(labels(all).length, 10);
  assert.equal(all.queryAll(node => node.tag === 'text' && node.props.fontSize === 12 && node.props.opacity === 0).length, 0, 'nothing is measured');
});

test('past 36 categories the bars are a list, not buttons; a row without a number draws no bar', async () => {
  const screen = await renderPage(page('onSelect={() => {}}', many(40, 'index === 3 ? null : index + 1')));
  assert.equal(screen.queryAll(node => node.tag === 'button').length, 0);
  const items = screen.queryAll(node => node.props.role === 'listitem');
  assert.equal(items.length, 40);
  assert.equal(items[3].props.accessibilityLabel, '3: ');
  assert.equal(bars(drawings(screen.getByTestId('chart'))[0].paths[0].d).length, 39);
});

test('states: a chosen bar set from outside; loading, empty and error replace the plot', async () => {
  const selected = await renderExample('BarChart/BarChart', 'StateSelected');
  const [chosen, others] = drawings(selected.getByTestId('selected'));
  assert.deepEqual([bars(chosen.paths[0].d).length, bars(others.paths[0].d).length], [1, 6]);
  assert.equal((await renderExample('BarChart/BarChart', 'StateLoading')).getByTestId('loading').props.stateBusy, 1);
  (await renderExample('BarChart/BarChart', 'StateEmpty')).getByText('No orders this week');
  const error = await renderExample('BarChart/BarChart', 'StateError');
  assert.equal(error.getByText('Orders could not be loaded.').props.role, 'alert');
  assert.equal(error.queryAll(node => node.tag === 'vector').length, 1, 'the error mark alone: no plot');
});
