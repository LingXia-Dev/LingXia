import { test } from 'node:test';
import assert from 'node:assert/strict';
import { defaultEnv, textOf } from '@lingxia/view/testing';
import { laidOut, renderExample, renderPage } from '../testing/harness.mjs';

/** A chart's drawings, in order: the bars, then those stepped back. */
const drawings = chart => find(chart, node => node.tag === 'vector' && node.props.opacity !== undefined)
  .map(node => ({ ...JSON.parse(node.props.vectorSource), opacity: node.props.opacity, marks: node.parent.props }));
const find = (node, test) => (test(node) ? [node] : node.children.flatMap(child => find(child, test)));
/** Each bar outline of a path as [x, free end, width]. */
const bars = d => [...d.matchAll(/M([\d.]+) [\d.]+V([\d.]+)h([\d.]+)V[\d.]+/g)].map(match => match.slice(1).map(Number));
/** Where each bar of a path stands: its zero line. */
const feet = d => [...d.matchAll(/M[\d.]+ ([\d.]+)V/g)].map(match => Number(match[1]));
/** The box that carries the zero line down the marks. */
const zeroBox = chart => find(chart, node => node.props.position === 'absolute' && node.children.length === 1 && node.children[0].props.background === 'line-strong')[0];
const slots = chart => find(chart, node => (node.tag === 'button' || node.props.role === 'listitem') && /: /.test(node.props.accessibilityLabel ?? ''));
/** How far down a box is moved, as a percentage of its own height, to two places. */
const down = node => Math.round(Number.parseFloat(JSON.parse(node.props.transform).translate[1]) * 100) / 100;
/** A slot's figure: how far down its mover stands, and the figure. */
const figure = slot => [down(slot.children[0]), textOf(slot.children[0])];
const count = node => 1 + node.children.reduce((sum, child) => sum + count(child), 0);
const page = (props, rows) => `
  import { BarChart } from '@liulidesign/charts/BarChart';
  const rows = ${rows};
  const money = value => '$' + value;
  export function render() { return <page><BarChart rows={rows} x="k" y="v" label="Chart" data-testid="chart" ${props} /></page>; }
`;
const many = (length, value = 'index + 1') => `Array.from({ length: ${length} }, (_, index) => ({ k: String(index), v: ${value}, w: ${length} - index }))`;

test('Orders: all seven bars are one outline in one drawing; each bar runs from its value to the foot and is open there', async () => {
  const screen = laidOut(await renderExample('BarChart/BarChart', 'Orders'));
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
  assert.ok(feet(outline.d).every(foot => foot === 100) && !/Z/.test(outline.d), 'unclosed at the foot, so the stroke stops flat there');
  assert.equal(outline.strokeLinecap, undefined, 'butt ends');
  // The marks start 3 pt under the figures' line, so the stroke that rounds a free end ends exactly at the value.
  assert.deepEqual([up.marks.insetTop, up.marks.insetBottom], [23, 0]);
});

test('Orders: each weekday is a button named by its day and value, carrying its figure level with its bar', async () => {
  const screen = laidOut(await renderExample('BarChart/BarChart', 'Orders'));
  const bar = day => screen.getByTestId(`orders-${day}`);
  assert.equal(bar('Thu').tag, 'button');
  assert.equal(bar('Thu').props.accessibilityLabel, 'Thu: 163');
  assert.equal(bar('Thu').props.statePressed, 0);
  assert.deepEqual(figure(bar('Thu')), [12.83, '163'], 'twenty-four of 187 above the figure, 163 under it');
  const [mover, lane] = bar('Thu').children;
  assert.deepEqual([mover.props.grow, lane.props.height], [1, 20], 'the plot\'s height less the figure\'s line, which is kept clear at the foot');
  const line = mover.children[0];
  assert.deepEqual([line.props.height, line.props.accessibilityHidden, line.props.alignSafe], [20, 1, 1], 'the button already reads the value');
  assert.deepEqual([line.children[0].props.fontVariantNumeric, line.children[0].props.fontSize, line.children[0].props.color, line.children[0].props.whiteSpace], ['tabular-nums', 13, 'text-muted', 'nowrap']);
  const marks = find(screen.getByTestId('orders'), node => node.props.position === 'absolute' && node.props.insetTop === 23)[0];
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
  const screen = laidOut(await renderExample('BarChart/BarChart', 'Grouped'));
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
  assert.deepEqual(figure(slots(chart())[0]), [18.42, '$31k'], 'one series of four bars: each bar carries its value, and the range refits');
  assert.equal(screen.getByTestId('channels-legend-online').props.statePressed, 0);
});

test('Signed: bars below zero hang from the same line in the same drawing, with their value under them', async () => {
  const screen = laidOut(await renderExample('BarChart/BarChart', 'Signed'));
  const chart = screen.getByTestId('cash');
  const [all, ...rest] = drawings(chart);
  assert.equal(rest.length, 0, 'one drawing for both sides of zero');
  assert.deepEqual(feet(all.paths[0].d), Array(6).fill(57.69), 'fifteen above zero and eleven below: zero lies 15 of 26 down');
  assert.deepEqual(bars(all.paths[0].d).map(bar => bar[1]), [11.54, 76.92, 26.92, 100, 46.15, 0], '12, -5, 8, -11, 3 and 15 on one scale');
  assert.deepEqual([all.marks.insetTop, all.marks.insetBottom], [23, 23], 'the corner\'s room at both free ends, inside the figures\' lines');
  assert.equal(down(zeroBox(chart)), 57.69, 'the zero line stands where the bars stand');
  const slot = name => slots(chart).find(node => node.props.accessibilityLabel === name);
  assert.deepEqual([figure(slot('Jan: $12k')), slot('Jan: $12k').props.paddingBottom], [[11.54, '$12k'], 20]);
  assert.deepEqual([figure(slot('Apr: -$11k')), slot('Apr: -$11k').props.paddingTop], [[100, '-$11k'], 20]);
});

test('new data that moves zero moves the zero line with the bars\' feet, on the bars\' own timing', async () => {
  const page = linked => `import { BarChart } from ${JSON.stringify(`${linked}/src/BarChart/BarChart`)};
import { useLocal } from '@lingxia/view';
const data = [[{ d: 'Mon', v: 9 }, { d: 'Tue', v: -3 }], [{ d: 'Mon', v: 2 }, { d: 'Tue', v: -6 }]];
export function render() {
  const step = useLocal('step', 0);
  return <page><button data-testid="next" onClick={() => step.set(1)}><text>Next</text></button><BarChart label="Net" rows={data[step.value]} x="d" y="v" data-testid="net" /></page>;
}
`;
  const screen = laidOut(await renderPage(page));
  const chart = () => screen.getByTestId('net');
  const read = () => [down(zeroBox(chart())), ...new Set(feet(drawings(chart())[0].paths[0].d))];
  assert.deepEqual(read(), [75, 75]);
  screen.press(screen.getByTestId('next'));
  assert.deepEqual(read(), [25, 25]);
  const timing = node => { const { properties, ...rest } = JSON.parse(node.props.transition); return rest; };
  const vector = find(chart(), node => node.tag === 'vector' && node.props.opacity !== undefined)[0];
  assert.deepEqual(timing(zeroBox(chart())), timing(vector), 'one duration, easing and delay');
  assert.ok(timing(vector).durationMs !== undefined && timing(vector).easing !== undefined, 'the pack\'s timing, not the default');
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
  assert.deepEqual([chosen.paths[0].strokeWidth, chosen.marks.insetTop], [2, 1]);
  const labels = screen.queryAll(node => node.tag === 'text' && / Jun$/.test(node.text ?? '') && node.props.fontSize === 12 && node.props.opacity !== 0);
  assert.deepEqual(labels.map(label => label.text), ['1 Jun', '30 Jun']);
  screen.getByText('15000');
});

test('a chart costs one element per bar and a fixed handful more, however many bars it has', async () => {
  const sized = async (length, props = '') => count((await renderPage(page(props, many(length)))).getByTestId('chart'));
  const [ten, sixty] = [await sized(10), await sized(60)];
  assert.equal(sixty - ten, 50, 'fifty more bars, fifty more elements');
  assert.ok(sixty <= 60 + 19, `${sixty} elements for 60 bars`);
  assert.ok(await sized(60, 'onChange={() => {}} value="7"') <= 60 + 23, 'a choice adds one drawing and the readout');
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
      return <page><BarChart rows={rows} x="k" y="v" label="Chart" data-testid="chart" /><button data-testid="again" onClick={() => tick.set(tick.value + 1)}>{String(tick.value)}</button></page>;
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
  const figures = screen => screen.queryAll(node => node.tag === 'text' && node.props.opacity !== 0 && /^\$/.test(node.text ?? '')).map(node => node.text);
  const auto = laidOut(await renderPage(page('formatValue={money}', rows)));
  assert.deepEqual(figures(auto), ['$200'], 'nine bars: the top of the scale only');
  const always = laidOut(await renderPage(page('formatValue={money} valueLabels="always"', rows)));
  assert.equal(figures(always).length, 9);
  const none = laidOut(await renderPage(page('formatValue={money} valueLabels="none"', rows)));
  assert.deepEqual(figures(none), []);
  assert.equal(bars(drawings(none.getByTestId('chart'))[0].paths[0].d)[1][1], 0, 'the tallest bar fills the plot');
  const tiny = laidOut(await renderPage(page('formatValue={money} size="xs"', many(8))));
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
  const empty = await renderPage(page('onChange={() => {}}', many(9)));
  assert.equal(readout(empty).length, 1);
  assert.equal(readout(empty)[0].props.minHeight, 18, 'the line keeps its height, so choosing does not move the plot');
  assert.equal(textOf(readout(empty)[0]), '');
  assert.equal(textOf(readout(await renderPage(page('value="4" formatValue={money}', many(9))))[0]), '4$5');
  const grouped = await renderPage(page('value="1"', many(9)).replace('y="v"', 'y={[{ field: "v", label: "V" }, { field: "w", label: "W" }]}'));
  assert.equal(textOf(readout(grouped)[0]), '1V2W8');
  assert.equal(readout(await renderPage(page('value="4" valueLabels="always"', many(9)))).length, 0, 'figures forced onto nine bars: no readout');
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
  const source = page('onChange={day.set} value={day.value}', many(40))
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
  const screen = await renderPage(page('onChange={() => {}}', many(40, 'index === 3 ? null : index + 1')));
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

test('bars in the hundred thousands, and below zero, lay out with small numbers', async () => {
  const screen = await renderPage(`
    import { BarChart } from '@liulidesign/charts/BarChart';
    const rows = [{ k: 'a', v: 98400 }, { k: 'b', v: -41200 }, { k: 'c', v: 141200 }];
    export function render() { return <page><BarChart rows={rows} x="k" y="v" label="Result by unit" valueLabels="always" data-testid="big" /></page>; }
  `);
  const placed = screen.queryAll(node => /transform/.test(node.props.transition ?? '')).map(down);
  assert.equal(placed.length, 4, 'each figure and the zero line by a share of the plot');
  assert.ok(placed.every(value => Number.isFinite(value) && value >= 0 && value <= 100), `shares of the plot, never past it: ${placed}`);
});

test('At large text: figures grow their line, and figures that would touch give way to the scale at the ends', async () => {
  const screen = await renderExample('BarChart/BarChart', 'Orders');
  const ghost = () => screen.queryAll(node => node.tag === 'text' && node.props.opacity === 0 && node.props.fontSize === 13)[0];
  const marks = () => drawings(screen.getByTestId('orders'))[0].marks;
  assert.equal(ghost().text, '132', 'the longest figure, laid out unseen');
  screen.layout(ghost().parent, { width: 308, height: 176 });
  screen.layout(ghost(), { width: 30, height: 34 });
  assert.equal(marks().insetTop, 37, 'the drawing starts under a figure as tall as it is drawn');
  assert.equal(screen.getByTestId('orders-Thu').children[1].props.height, 34);
  screen.layout(ghost(), { width: 42, height: 34 });
  assert.equal(screen.queryAll(node => node.tag === 'text' && node.text === '163').length, 0, 'seven 42 pt figures do not fit in 308 pt');
  assert.equal(marks().insetTop, 3);
  screen.getByText('200');
});

test('before the plot is laid out every bar stands at zero, keyed by its series, so the bars grow to their values', async () => {
  const screen = await renderExample('BarChart/BarChart', 'Orders');
  const [before] = drawings(screen.getByTestId('orders'));
  assert.ok(before.paths[0].d.split('M').slice(1).every(bar => /^[\d.]+ 100V100h[\d.]+V100$/.test(bar)), before.paths[0].d.slice(0, 80));
  laidOut(screen);
  const [after] = drawings(screen.getByTestId('orders'));
  assert.equal(after.paths[0].key, before.paths[0].key);
  assert.equal(after.paths[0].d.split('M').length, before.paths[0].d.split('M').length, 'the same commands, so the bars move');
  assert.notEqual(after.paths[0].d, before.paths[0].d);
});

test('a bar chart that comes back with the same rows stands at its values from its first frame', async () => {
  const page = linked => `import { BarChart } from ${JSON.stringify(`${linked}/src/BarChart/BarChart`)};
import { useLocal } from '@lingxia/view';
const rows = [{ day: 'Mon', orders: 3 }, { day: 'Tue', orders: 5 }];
export function render() {
  const on = useLocal('on', true);
  return <page><button data-testid="toggle" onClick={() => on.set(!on.value)}><text>Toggle</text></button>{on.value && <BarChart label="Orders" rows={rows} x="day" y="orders" data-testid="orders" />}</page>;
}
`;
  const screen = laidOut(await renderPage(page));
  const laid = drawings(screen.getByTestId('orders'))[0].paths[0].d;
  screen.press(screen.getByTestId('toggle'));
  screen.press(screen.getByTestId('toggle'));
  assert.equal(drawings(screen.getByTestId('orders'))[0].paths[0].d, laid, 'not grown again');
});

test('a figure moves with its bar\'s free end: the same start, end and timing, so level with it at every point of a change', async () => {
  const page = linked => `import { BarChart } from ${JSON.stringify(`${linked}/src/BarChart/BarChart`)};
import { useLocal } from '@lingxia/view';
const data = [[{ d: 'Mon', v: 3 }, { d: 'Tue', v: 5 }], [{ d: 'Mon', v: 8 }, { d: 'Tue', v: 2 }]];
export function render() {
  const step = useLocal('step', 0);
  return <page><button data-testid="next" onClick={() => step.set(1)}><text>Next</text></button><BarChart label="Orders" rows={data[step.value]} x="d" y="v" data-testid="orders" /></page>;
}
`;
  const screen = await renderPage(page);
  const timing = node => { const { properties, ...rest } = JSON.parse(node.props.transition); return rest; };
  const place = node => Number.parseFloat(JSON.parse(node.props.transform).translate[1]);
  const vector = () => find(screen.getByTestId('orders'), node => node.tag === 'vector' && node.props.opacity !== undefined)[0];
  const read = () => [slots(screen.getByTestId('orders')).map(slot => place(slot.children[0])), bars(JSON.parse(vector().props.vectorSource).paths[0].d).map(bar => bar[1])];
  const [unlaid] = read();
  assert.deepEqual(unlaid, [100, 100], 'before layout the figures stand at zero with their bars');
  laidOut(screen);
  const [grown, tops] = read();
  screen.press(screen.getByTestId('next'));
  const [moved, newTops] = read();
  for (const [figures, ends] of [[grown, tops], [moved, newTops]]) assert.ok(figures.every((at, index) => Math.abs(at - ends[index]) < 0.01), `${figures} ${ends}`);
  assert.notDeepEqual(moved, grown, 'the figures moved');
  assert.deepEqual(timing(slots(screen.getByTestId('orders'))[0].children[0]), timing(vector()), 'one duration, easing and delay');
});

test('with reduced motion a figure takes its bar\'s new place at once', async () => {
  const screen = laidOut(await renderExample('BarChart/BarChart', 'Orders', { env: defaultEnv({ reducedMotion: true }) }));
  assert.deepEqual(screen.queryAll(node => /transform|"d"/.test(node.props.transition ?? '')).map(node => node.tag), []);
  assert.equal(figure(screen.getByTestId('orders-Thu'))[0], 12.83);
});

/** Gives every measured text a size, one at a time, as a host does: 7 pt a character, 16 pt high. */
const measured = screen => {
  const done = new Set();
  for (;;) {
    const node = screen.queryAll(each => each.tag === 'text' && each.props.onLayout !== undefined && !done.has(`${each.props.onLayout}:${textOf(each)}`))[0];
    if (!node) return screen;
    done.add(`${node.props.onLayout}:${textOf(node)}`);
    screen.layout(node, { width: textOf(node).length * 7, height: 16 });
  }
};
/** A reference path's gaps, in its own units: [slot, start, end], one per slot. */
const gapsOf = d => [...d.matchAll(/H([\d.]+)M([\d.]+) /g)].map(([, start, end], slot) => [slot, Number(start), Number(end)]);
/** The gaps with a width, in points on a 320 pt plot. */
const openGaps = d => gapsOf(d).filter(([, start, end]) => end > start).map(([slot, start, end]) => [slot, start * 3.2, end * 3.2]);
const referenceLabels = screen => screen.queryAll(node => node.tag === 'text' && node.props.insetInlineEnd !== undefined);
const sideOf = label => (label.props.insetBottom === 0 ? 'above' : 'below');

test('Target: dashed lines on the bars\' timing, the range widened to hold them, labels at the end clear of the last bars\' figures', async () => {
  const screen = measured(laidOut(await renderExample('BarChart/BarChart', 'Target')));
  const chart = screen.getByTestId('target');
  const vectors = find(chart, node => node.tag === 'vector');
  const vector = vectors.find(node => /reference-/.test(node.props.vectorSource));
  assert.equal(vectors.indexOf(vector), 0, 'the lines lie under the bars and their figures');
  const paths = JSON.parse(vector.props.vectorSource).paths;
  // 160 and 132 on a scale to 187. On a 320 by 160 plot the Target line passes through Mon's figure (132) and
  // Tue's (148), the Average line through Wed's (121): each breaks for that figure's text box, 2 pt wider each side.
  const gaps = paths.map(path => openGaps(path.d));
  assert.deepEqual(gaps.map(each => each.map(([slot]) => slot)), [[0, 1], [2]], 'Target through Mon and Tue, Average through Wed');
  // Each figure three characters, measured at 7 pt a character.
  gaps.flat().forEach(([slot, start, end]) => {
    const middle = (slot + 0.5) * (320 / 7);
    assert.ok(Math.abs(end - start - (3 * 7 + 4)) < 0.05 && Math.abs((start + end) / 2 - middle) < 0.05, `gap at ${slot}: ${start}..${end}`);
  });
  assert.equal(vector.props.transition, vectors[1].props.transition, 'the lines move with the bars');
  assert.ok(paths.every(path => gapsOf(path.d).length === 7), 'one gap per bar, empty ones too');
  const labels = referenceLabels(screen);
  assert.deepEqual(labels.map(textOf), ['Target 160', 'Average 132']);
  const timing = node => { const { durationMs, easing } = JSON.parse(node.props.transition); return JSON.stringify([durationMs, easing]); };
  assert.ok(labels.every(label => timing(label.parent.parent) === timing(vector)), 'each label moves with its line');
  // Sun's figure, 74, stands far below both lines; Sat's, 96, too: the labels keep their first place.
  assert.deepEqual(labels.map(sideOf), ['above', 'above']);
  screen.getByText('Target, 160');
  screen.getByText('Average, 132');
});

test('a reference level with the last bar\'s figure moves its label below the line', async () => {
  const rows = `[{ k: 'a', v: 40 }, { k: 'b', v: 60 }, { k: 'c', v: 100 }]`;
  const screen = measured(laidOut(await renderPage(page(`references={[{ value: 98, label: 'Capacity' }]}`, rows))));
  assert.deepEqual(referenceLabels(screen).map(label => [textOf(label), sideOf(label)]), [['Capacity 98', 'below']]);
  const clear = measured(laidOut(await renderPage(page(`references={[{ value: 98, label: 'Capacity' }]}`, `[{ k: 'a', v: 100 }, { k: 'b', v: 60 }, { k: 'c', v: 40 }]`))));
  assert.deepEqual(referenceLabels(clear).map(sideOf), ['above'], 'the tall bar is at the start, away from the label');
});

test('references 2% apart on signed bars: the second below its line; right to left too, the labels keep to the plot\'s end edge, its right', async () => {
  const rows = `[{ k: 'a', v: -50 }, { k: 'b', v: 10 }, { k: 'c', v: 20 }, { k: 'd', v: 30 }, { k: 'e', v: 40 }, { k: 'f', v: 50 }, { k: 'g', v: 60 }, { k: 'h', v: 70 }, { k: 'i', v: 80 }, { k: 'j', v: 30 }]`;
  for (const direction of ['ltr', 'rtl']) {
    const screen = measured(laidOut(await renderPage(page(`references={[{ value: 0, label: 'Plan' }, { value: -2.6, label: 'Last year' }]}`, rows), { env: { ...defaultEnv, direction } })));
    const labels = referenceLabels(screen);
    assert.deepEqual(labels.map(label => [textOf(label), sideOf(label)]), [['Plan 0', 'above'], ['Last year -2.6', 'below']], direction);
    assert.ok(labels.every(label => label.props.insetInlineEnd === 0 && label.props.insetInlineStart === undefined));
  }
});

test('a reference line below zero breaks for the figure under a bar below zero', async () => {
  const screen = measured(laidOut(await renderPage(page(`references={[{ value: -57, label: 'Floor' }]}`, `[{ k: 'a', v: -50 }, { k: 'b', v: 50 }]`))));
  const vectors = find(screen.getByTestId('chart'), node => node.tag === 'vector');
  const [line] = JSON.parse(vectors[0].props.vectorSource).paths;
  assert.equal(line.key, 'reference-0');
  const [[slot, start, end]] = openGaps(line.d);
  assert.equal(slot, 0);
  assert.ok(Math.abs(end - start - (3 * 7 + 4)) < 0.05 && Math.abs((start + end) / 2 - 80) < 0.05, `"-50" is 21 pt wide at a's middle: ${start}..${end}`);
});

test('new signed rows: the bars\' outline keeps its commands, so each bar moves from its old value to its new one through zero, on a declared transition', async () => {
  const page = linked => `import { BarChart } from ${JSON.stringify(`${linked}/src/BarChart/BarChart`)};
import { useLocal } from '@lingxia/view';
const data = [[{ m: 'Jan', v: 12 }, { m: 'Feb', v: -5 }, { m: 'Mar', v: 8 }], [{ m: 'Jan', v: -8 }, { m: 'Feb', v: 9 }, { m: 'Mar', v: -14 }]];
export function render() {
  const step = useLocal('step', 0);
  return <page><button data-testid="next" onClick={() => step.set(1)}><text>Next</text></button><BarChart label="Net" rows={data[step.value]} x="m" y="v" data-testid="net" /></page>;
}
`;
  const screen = laidOut(await renderPage(page));
  const vector = () => drawings(screen.getByTestId('net'))[0];
  const node = () => find(screen.getByTestId('net'), each => each.tag === 'vector' && each.props.opacity !== undefined)[0];
  const commands = d => d.replace(/[\d.-]+/g, '').replace(/\s+/g, '');
  const before = vector().paths[0].d;
  screen.press(screen.getByTestId('next'));
  const after = vector().paths[0].d;
  assert.notEqual(after, before);
  assert.equal(commands(after), commands(before), 'same commands, so the outline interpolates bar by bar');
  const ends = d => bars(d).map(([, end]) => end);
  const [zeroBefore, zeroAfter] = [feet(before)[0], feet(after)[0]];
  assert.deepEqual(ends(before).map(end => Math.sign(zeroBefore - end)), [1, -1, 1]);
  assert.deepEqual(ends(after).map(end => Math.sign(zeroAfter - end)), [-1, 1, -1], 'each bar now on the other side of zero');
  const { properties, durationMs } = JSON.parse(node().props.transition);
  assert.ok(properties.includes('d') && durationMs !== 0, node().props.transition);
});

test('new rows: each reference line keeps its commands, its gaps opening and closing on the bars\' transition, so it never snaps', async () => {
  const page = linked => `import { BarChart } from ${JSON.stringify(`${linked}/src/BarChart/BarChart`)};
import { useLocal } from '@lingxia/view';
const data = [[{ m: 'Jan', v: 125 }, { m: 'Feb', v: 60 }, { m: 'Mar', v: 90 }, { m: 'Apr', v: 152 }], [{ m: 'Jan', v: 60 }, { m: 'Feb', v: 125 }, { m: 'Mar', v: 90 }, { m: 'Apr', v: 152 }]];
export function render() {
  const step = useLocal('step', 0);
  return <page><button data-testid="next" onClick={() => step.set(1)}><text>Next</text></button><BarChart label="Net" rows={data[step.value]} x="m" y="v" references={[{ value: 140, label: 'Goal' }]} data-testid="net" /></page>;
}
`;
  const screen = measured(laidOut(await renderPage(page)));
  const line = () => find(screen.getByTestId('net'), node => node.tag === 'vector' && /reference-/.test(node.props.vectorSource))[0];
  const commands = d => d.replace(/[\d.-]+/g, '').replace(/\s+/g, '');
  const before = JSON.parse(line().props.vectorSource).paths[0].d;
  screen.press(screen.getByTestId('next'));
  measured(screen);
  const after = JSON.parse(line().props.vectorSource).paths[0].d;
  assert.deepEqual([openGaps(before).map(([slot]) => slot), openGaps(after).map(([slot]) => slot)], [[0], [1]], 'the gap moved from Jan to Feb');
  assert.equal(commands(after), commands(before));
  assert.ok(JSON.parse(line().props.transition).properties.includes('d'));
});

test('right to left, the plot keeps time left to right: bars, figures, press slots, x labels and a line\'s gaps all agree', async () => {
  const direction = node => { for (let at = node; at; at = at.parent) if (at.props.direction) return at.props.direction; return 'page'; };
  for (const env of [defaultEnv, { ...defaultEnv, direction: 'rtl' }]) {
    const screen = measured(laidOut(await renderExample('BarChart/BarChart', 'Target', { env })));
    const chart = screen.getByTestId('target');
    const [slot] = slots(chart);
    const ticks = screen.queryAll(node => node.tag === 'text' && textOf(node) === 'Mon');
    assert.deepEqual([direction(slot), ...ticks.map(direction)], ['ltr', 'ltr', 'ltr'], `${env.direction ?? 'ltr'}: slots and labels in a left-to-right row`);
    assert.deepEqual(slots(chart).map(each => each.props.accessibilityLabel.split(':')[0]), ['Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat', 'Sun'], 'in time order');
    // Slot i spans [i, i + 1] sevenths of the plot; bar i and its figure's gap are centred in it.
    const [drawing] = drawings(chart);
    const middles = bars(drawing.paths[0].d).map(([x, , width]) => x + width / 2);
    middles.forEach((middle, index) => assert.ok(Math.abs(middle - ((index + 0.5) * 100) / 7) < 0.05, `bar ${index} at ${middle}`));
    const line = JSON.parse(find(chart, node => node.tag === 'vector' && /reference-/.test(node.props.vectorSource))[0].props.vectorSource).paths[0];
    for (const [index, start, end] of openGaps(line.d)) assert.ok(Math.abs((start + end) / 2 / 3.2 - middles[index]) < 0.05, `gap ${index}`);
  }
});
