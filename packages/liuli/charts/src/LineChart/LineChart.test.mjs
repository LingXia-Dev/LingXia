import { test } from 'node:test';
import assert from 'node:assert/strict';
import { defaultEnv, textOf } from '@lingxia/view/testing';
import { laidOut, renderExample, renderPage } from '../testing/harness.mjs';

// The plot's drawings: the fills under the lines, the lines, and the dots over them; the marker is absolute.
const drawings = screen => screen.queryAll(node => node.tag === 'vector' && node.props.grow === 1 && node.props.position !== 'absolute');
const lineVector = screen => drawings(screen).find(node => node.parent.props.position !== 'absolute');
const drawing = screen => ({ ...JSON.parse((lineVector(screen) ?? drawings(screen)[0]).props.vectorSource), paths: drawings(screen).flatMap(node => JSON.parse(node.props.vectorSource).paths) });
const points = d => [...d.matchAll(/(-?[\d.]+) (-?[\d.]+)/g)].map(match => [Number(match[1]), Number(match[2])]);
const unread = node => (node ? Boolean(node.props.accessibilityHidden) || unread(node.parent) : false);
const reading = (screen, label) => screen.queryAll(node => node.props.role === 'list' && node.props.accessibilityLabel === label)[0];

test('Weight: the line fits its data edge to edge, the latest value is dotted, and the drawing is hidden', async () => {
  const screen = laidOut(await renderExample('LineChart/LineChart', 'Weight'));
  assert.equal(screen.getByTestId('weight').props.accessibilityLabel, 'Weight by week');
  const vector = lineVector(screen);
  assert.equal(vector.props.accessibilityHidden, 1);
  assert.equal(vector.props.widthPercent, 100, 'a vector keeps its own 24 pt until sized');
  const { paths, fit } = drawing(screen);
  assert.equal(fit, 'fill');
  const [line, end] = paths;
  const heights = points(line.d).map(point => point[1]);
  assert.deepEqual([Math.min(...heights), Math.max(...heights)], [5, 100], 'the plot runs from 70.5 to 72.5 kg, so two kilos of change fill it');
  assert.deepEqual([points(line.d)[0][0], points(line.d).at(-1)[0]], [0, 100]);
  assert.equal(line.stroke, 'chart-1');
  assert.equal(line.vectorEffect, 'non-scaling-stroke');
  assert.deepEqual([line.pathLength, line.strokeDasharray, line.strokeDashoffset], [1, [1], 0], 'laid out, the line is drawn whole');
  assert.equal(end.d, 'M100 100h0.01');
  assert.ok(end.strokeWidth > line.strokeWidth);
});

test('Smooth: without the end label no dot sits on the latest value at rest; asked for, it does', async () => {
  const screen = await renderExample('LineChart/LineChart', 'Smooth');
  assert.equal(drawing(screen).paths.length, 1, 'the line alone');
  const asked = await renderPage(`
    import { LineChart } from '@liulidesign/charts/LineChart';
    const rows = [{ t: 'a', v: 1 }, { t: 'b', v: 3 }, { t: 'c', v: 2 }];
    export function render() { return <page><LineChart rows={rows} x="t" y="v" label="V" hasEndDot /></page>; }`);
  assert.equal(drawing(asked).paths.at(-1).d, 'M100 50h0.01');
});

test('Weight: the summary, then every row as a list item with its formatted value; the end labels are not read', async () => {
  const screen = await renderExample('LineChart/LineChart', 'Weight');
  screen.getByText('8 values from 6 Oct to 24 Nov. Lowest 70.5 kg at 24 Nov, highest 72.4 kg at 6 Oct. Falling: 72.4 kg first, 70.5 kg last.');
  const list = reading(screen, 'Weight by week');
  assert.equal(list.props.opacity, 0, 'for assistive tech only');
  assert.equal(list.children.length, 8);
  assert.ok(list.children.every(item => item.props.role === 'listitem'));
  assert.equal(textOf(list.children[4]), '3 Nov: 71.2 kg');
  const ends = screen.queryAll(node => node.tag === 'text' && node.props.fontSize === 12 && node.props.opacity !== 0).map(node => node.text);
  assert.deepEqual(ends, ['6 Oct', '24 Nov']);
  assert.ok(unread(screen.getByText('24 Nov')));
});

/** A column's movers: boxes as tall as the plot, each moved down to its tick. */
const movers = column => column.children.find(node => node.props.position === 'relative')?.children ?? [];
const ticks = column => movers(column).map(mover => mover.children[0]);
/** How far down each mover is, as a percentage of the plot's height, to two places. */
const down = node => Math.round(Number.parseFloat(JSON.parse(node.props.transform).translate[1]) * 100) / 100;
const levels = column => movers(column).map(down);

test('Weight: the scale is round numbers at the start edge, each level with a hairline across the plot; the latest value sits at the line\'s end', async () => {
  const screen = await renderExample('LineChart/LineChart', 'Weight');
  const frame = lineVector(screen).parent.parent;
  const [scale, plot, end] = frame.children;
  assert.deepEqual(ticks(scale).map(textOf), ['72.5 kg', '72.0 kg', '71.5 kg', '71.0 kg', ''], 'half a kilo apart; the foot is 70.5 kg, which the end figure says on the same line');
  assert.deepEqual(levels(scale), [0, 25, 50, 75, 100], 'each label stands at its value\'s share of the range');
  assert.ok(unread(ticks(scale)[0]), 'the same numbers are in the summary and the list');
  assert.deepEqual(scale.children[0].children.map(textOf), ['72.5 kg', '72.0 kg', '71.5 kg', '71.0 kg', ''], 'an unseen copy, in the flow, gives the column its width');
  const labels = ticks(scale).map(tick => tick.children[0]);
  assert.deepEqual(labels.map(label => [label.props.fontSize, label.props.color, label.props.fontVariantNumeric]), Array(5).fill([13, 'text-muted', 'tabular-nums']));
  assert.ok(labels.every(label => JSON.parse(label.props.transform).translate[1] === '-50%'), 'each label is centred on its line');
  const grid = plot.children.find(node => node.props.position === 'absolute' && ticks(node).length === 5);
  assert.deepEqual(levels(grid), levels(scale), 'the hairlines stand where the labels do');
  assert.deepEqual(ticks(grid).map(tick => [tick.children[0].props.height, tick.children[0].props.background]), [...Array(4).fill([1, 'text/8']), [1, 'text/12']], 'quiet lines, a firmer one under the values');
  // Scale, hairlines and end figure share the drawing's 4 pt inset, so a label is level with its line and its values.
  assert.deepEqual([scale, grid, end].map(node => [node.props.paddingTop, node.props.paddingBottom]), Array(3).fill([4, 4]));
  // The drawing is cut at its viewBox, which reaches 4 pt past the data on every side, so the end dot is whole.
  const reach = () => { const [x, y, width, height] = drawing(screen).viewBox; return [x, y, width, height].map(value => Math.round(value * 100) / 100); };
  assert.deepEqual(reach(), [-1.37, -2.38, 102.74, 104.76], 'until the plot is measured: 300 pt wide, and 176 pt high at this size');
  screen.layout(plot, { width: 208, height: 176 });
  assert.deepEqual(reach(), [-2, -2.38, 104, 104.76], '4 of 200 pt is 2 of 100');
  const [mover] = end.children;
  assert.deepEqual([mover.props.grow, down(mover), textOf(mover)], [1, 100, '70.5 kg'], 'the plot\'s height, moved all the way down: the latest value is the foot');
  assert.equal(mover.children[0].props.height, 0);
  assert.equal(JSON.parse(mover.children[0].children[0].props.transform).translate[1], '-50%', 'the figure is centred on the value\'s height');
  const ghosts = screen.getByText('6 Oct').parent.parent.parent.children.filter(node => node.props.height === 0);
  assert.deepEqual(ghosts.map(textOf), ['72.5 kg72.0 kg71.5 kg71.0 kg', '70.5 kg'], 'unseen copies of the scale and the end figure inset the category labels as much');
});

test('the scale fits the data: an area from zero stops just past its highest value, and a short plot has fewer ticks', async () => {
  const page = props => `
    import { LineChart } from '@liulidesign/charts/LineChart';
    const rows = [4120, 3980, 4460, 4710, 5240, 6130, 5380].map((v, index) => ({ t: 'd' + index, v }));
    export function render() { return <page><LineChart rows={rows} x="t" y="v" label="Chart" hasArea xLabels="none" ${props} /></page>; }
  `;
  const scaleOf = screen => lineVector(screen).parent.parent.children[0];
  const medium = scaleOf(await renderPage(page('')));
  assert.deepEqual(ticks(medium).map(textOf), ['6000', '4000', '2000', '0'], 'two thousand apart');
  assert.deepEqual(levels(medium), [7.69, 38.46, 69.23, 100], 'the plot ends at 6500, a quarter step past 6130, not at 8000');
  const small = scaleOf(await renderPage(page('size="sm"')));
  assert.deepEqual(ticks(small).map(textOf), ['5000', '0'], 'two ticks in 112 pt');
  assert.deepEqual(levels(small), [20, 100]);
});

test('hasScale off fits the data exactly; the smallest size is bare; several series get the scale but no end figure', async () => {
  const page = props => `
    import { LineChart } from '@liulidesign/charts/LineChart';
    const rows = [{ t: 'a', v: 3, w: 40 }, { t: 'b', v: 7, w: 20 }, { t: 'c', v: 5, w: 30 }];
    export function render() { return <page><LineChart rows={rows} x="t" label="Chart" ${props} /></page>; }
  `;
  const texts = screen => screen.queryAll(node => node.tag === 'text' && node.props.role !== 'listitem' && node.props.opacity !== 0).map(node => node.text);
  const bare = await renderPage(page('y="v" hasScale={false}'));
  assert.deepEqual(points(drawing(bare).paths[0].d).map(point => point[1]), [100, 0, 50]);
  assert.deepEqual(texts(bare), ['a', 'c']);
  assert.deepEqual(texts(await renderPage(page('y="v" size="xs" hasEndLabel'))).filter(text => /\d/.test(text)), ['5']);
  const two = await renderPage(page('y={[{ field: "v", label: "V" }, { field: "w", label: "W" }]} hasEndLabel legend="none"'));
  assert.deepEqual(texts(two).filter(text => /\d/.test(text)), Array(3).fill(['40', '30', '20', '10']).flat(), 'the scale, its unseen copy for its own width and one for the category labels; no end figure for two lines');
  const apart = await renderPage(page('y="w" hasEndLabel'));
  const scale = ['40', '35', '', '25', '20'];
  assert.deepEqual(texts(apart).filter(text => /\d/.test(text)), [...scale, ...scale, '30', ...scale, '30'].filter(Boolean), 'the tick the end figure stands on is left to the end figure');
});

test('Area: one series is filled down to the foot and measured from zero', async () => {
  const screen = laidOut(await renderExample('LineChart/LineChart', 'Area'));
  const [fill, line] = drawing(screen).paths;
  assert.equal(fill.fill, 'chart-1/16');
  assert.match(fill.d, /L100 100L0 100Z$/);
  assert.doesNotMatch(line.d, /C/, 'daily counts are joined by straight segments');
  assert.ok(Math.max(...points(line.d).map(point => point[1])) < 75, 'the lowest day is well above zero');
  assert.equal(lineVector(screen).props.paddingBottom, undefined, 'the fill reaches the rule');
});

test('Series: lines differ by dash as well as colour, and the legend repeats both and hides a series', async () => {
  const screen = laidOut(await renderExample('LineChart/LineChart', 'Series'));
  const lines = () => drawing(screen).paths.filter(path => path.strokeWidth === 2);
  assert.deepEqual(lines().map(path => path.stroke), ['chart-1', 'chart-2', 'chart-3']);
  assert.deepEqual(lines().map(path => path.strokeDasharray), [[1], [6, 4], [2, 3]], 'the solid line draws in; dashed lines keep their dashes');
  const swatch = key => JSON.parse(screen.getByTestId(`plans-legend-${key}`).children[0].props.vectorSource).paths[0];
  assert.deepEqual([swatch('team').stroke, swatch('team').strokeDasharray], ['chart-2', [6, 4]]);
  assert.equal(textOf(reading(screen, 'Accounts by plan and month').children[3]), 'Apr: Free 492, Team 171, Pro 88');

  screen.press(screen.getByTestId('plans-legend-free'));
  assert.deepEqual(lines().map(path => path.stroke), ['chart-2', 'chart-3']);
  assert.deepEqual(lines().map(path => path.strokeDasharray), [[6, 4], [2, 3]], 'a series keeps its dash when another is hidden');
  assert.equal(textOf(reading(screen, 'Accounts by plan and month').children[3]), 'Apr: Team 171, Pro 88');
  assert.equal(screen.getByTestId('plans-legend-free').props.statePressed, 0);
});

const marker = screen => JSON.parse(screen.queryAll(node => node.tag === 'vector' && node.props.position === 'absolute')[0].props.vectorSource).paths;
const readout = screen => textOf(screen.queryAll(node => node.props.role === 'status')[0]);
const plot = { width: 308, height: 112 };
const touch = (screen, phase, x) => screen.pointer(screen.getByTestId('picked-plot'), { phase, x, y: 40, ...plot });

test('Selected: the chosen row gets a rule and a dot on each line in a drawing of its own, and the readout names it', async () => {
  const screen = await renderExample('LineChart/LineChart', 'Selected');
  assert.equal(marker(screen)[0].d, 'M60 0V100', 'April is the fourth of six');
  assert.equal(marker(screen)[0].stroke, 'text/20');
  const dots = marker(screen).filter(path => path.strokeWidth === 8);
  assert.equal(dots.length, 3);
  assert.ok(dots.every(dot => dot.d.startsWith('M60 ')));
  assert.equal(readout(screen), 'AprFree492Team171Pro88');
  assert.equal(drawing(screen).paths.some(path => path.strokeWidth === 8 || path.stroke === 'text/20'), false, 'the lines hold no marker');
  assert.equal(screen.queryAll(node => node.props.role === 'list' && node.props.accessibilityLabel === 'Accounts by plan and month').length, 1, 'no legend: only the reading');
});

test('Selected: a drag across the plot chooses the nearest row; a touch that only lands chooses nothing, so a scroll can still take it', async () => {
  const screen = await renderExample('LineChart/LineChart', 'Selected');
  const lines = drawing(screen).paths;
  assert.equal(screen.getByTestId('picked-plot').props.pointerAxis, 'x', 'vertical movement stays with the page');
  touch(screen, 'down', 20);
  assert.equal(readout(screen), 'AprFree492Team171Pro88', 'landing changes nothing');
  screen.pointer(screen.getByTestId('picked-plot'), { phase: 'move', x: 24, y: 90, ...plot });
  assert.equal(readout(screen), 'AprFree492Team171Pro88', 'a drag that runs more down than across is the page\'s scroll');
  touch(screen, 'move', 70);
  assert.equal(readout(screen), 'FebFree455Team131Pro71');
  touch(screen, 'cancel', 70);
  assert.equal(readout(screen), 'AprFree492Team171Pro88', 'a press the page takes after all puts the choice back');
  touch(screen, 'down', 20);
  touch(screen, 'move', 70);
  assert.equal(readout(screen), 'FebFree455Team131Pro71', 'six points across 300 pt inside the 4 pt inset: 70 is nearest the second');
  assert.equal(marker(screen)[0].d, 'M20 0V100');
  touch(screen, 'move', 400);
  assert.equal(readout(screen), 'JunFree561Team236Pro121', 'past the edge it stays on the last row');
  touch(screen, 'up', -30);
  assert.equal(readout(screen), 'JanFree410Team120Pro64');
  assert.deepEqual(drawing(screen).paths, lines, 'a scrub redraws the marker, never the lines');
  touch(screen, 'up', 184);
  assert.equal(readout(screen), 'AprFree492Team171Pro88', 'a tap chooses where it lifts');
});

test('Selected: a mouse chooses as it moves and clears as it leaves; arrows and a screen reader step from the chosen row', async () => {
  const screen = await renderExample('LineChart/LineChart', 'Selected');
  const scrubber = () => screen.getByTestId('picked-plot');
  assert.equal(scrubber().props.accessibilityLabel, 'Accounts by plan and month');
  assert.deepEqual([scrubber().props.ariaValueNow, scrubber().props.ariaValueMin, scrubber().props.ariaValueMax, scrubber().props.ariaValueText], [3, 0, 5, 'Apr: Free 492, Team 171, Pro 88']);
  screen.pointer(scrubber(), { phase: 'hover', x: 4, y: 10, ...plot, pointer: 'mouse' });
  assert.equal(readout(screen), 'JanFree410Team120Pro64');
  screen.pointer(scrubber(), { phase: 'leave', x: -5, y: 10, ...plot, pointer: 'mouse' });
  assert.equal(readout(screen), '', 'the line is kept, empty');
  assert.equal(screen.queryAll(node => node.tag === 'vector' && node.props.position === 'absolute').length, 0, 'no marker');
  const step = phase => screen.pointer(scrubber(), { phase, x: 0, y: 0, width: 0, height: 0, pointer: 'keyboard' });
  step('decrement');
  assert.equal(readout(screen), 'MayFree530Team204Pro97', 'with nothing chosen it rests on the latest row, so one step back is May');
  step('increment');
  step('increment');
  assert.equal(readout(screen), 'JunFree561Team236Pro121', 'and stops at the end');
});

test('category labels: every label that fits is written under its point; a narrow chart thins them evenly and keeps the ends when it can', async () => {
  const page = (count, props = '') => `
    import { LineChart } from '@liulidesign/charts/LineChart';
    const rows = Array.from({ length: ${count} }, (_, index) => ({ t: 'Day ' + (index + 1), v: index }));
    export function render() { return <page><LineChart rows={rows} x="t" y="v" label="Chart" ${props} /></page>; }
  `;
  const labels = screen => screen.queryAll(node => node.tag === 'text' && node.props.fontSize === 12 && node.props.opacity !== 0);
  const measure = (screen, width, tick) => {
    const ghost = screen.queryAll(node => node.tag === 'text' && node.props.fontSize === 12 && node.props.opacity === 0)[0];
    screen.layout(ghost.parent, { width, height: 16 });
    screen.layout(screen.queryAll(node => node.tag === 'text' && node.props.fontSize === 12 && node.props.opacity === 0)[0], { width: tick, height: 16 });
  };
  const seven = await renderPage(page(7));
  assert.deepEqual(labels(seven).map(node => node.text), ['Day 1', 'Day 7'], 'before the row is measured: the ends');
  measure(seven, 308, 28);
  assert.deepEqual(labels(seven).map(node => node.text), ['Day 1', 'Day 2', 'Day 3', 'Day 4', 'Day 5', 'Day 6', 'Day 7']);
  assert.deepEqual(labels(seven).map(node => node.props.insetLeft), [0, 40, 90, 140, 190, 240, 280], 'each centred on its point, 50 pt apart inside the 4 pt inset; the ends held inside the row');
  assert.deepEqual(labels(seven).map(node => node.props.textAlign), ['start', 'center', 'center', 'center', 'center', 'center', 'end']);
  assert.ok(labels(seven).every(node => node.props.width === 28));
  measure(seven, 308, 36);
  assert.deepEqual(labels(seven).map(node => node.text), ['Day 1', 'Day 3', 'Day 5', 'Day 7'], 'an end label is held half its width nearer its neighbour, so every second: it reaches both ends');
  const eight = await renderPage(page(8));
  measure(eight, 308, 40);
  assert.deepEqual(labels(eight).map(node => node.text), ['Day 2', 'Day 4', 'Day 6', 'Day 8'], 'no step reaches both ends of eight, so the count runs back from the latest');
  const ends = await renderPage(page(7, 'xLabels="ends"'));
  assert.equal(ends.queryAll(node => node.tag === 'text' && node.props.opacity === 0 && node.props.fontSize === 12).length, 0, 'nothing is measured');
  assert.deepEqual(labels(ends).map(node => node.text), ['Day 1', 'Day 7']);
});

test('Gaps: a row without a number breaks the line and is left out of the reading', async () => {
  const screen = await renderExample('LineChart/LineChart', 'Gaps');
  const [line] = drawing(screen).paths;
  assert.equal(line.d.split('M').filter(Boolean).length, 3);
  const items = reading(screen, 'Temperature by hour').children.map(textOf);
  assert.equal(items.length, 7);
  assert.ok(!items.some(item => item.startsWith('08:00')));
});

test('a long series is reduced to 240 points and read by its summary alone', async () => {
  const screen = await renderPage(`
    import { LineChart } from '@liulidesign/charts/LineChart';
    import { summarize } from '@liulidesign/charts/summary/en';
    const rows = Array.from({ length: 1000 }, (_, index) => ({ t: String(index), v: index === 500 ? 999 : index % 20 }));
    export function render() { return <page><LineChart rows={rows} x="t" y="v" label="Load" summarize={summarize} data-testid="load" /></page>; }
  `);
  const [line] = drawing(screen).paths;
  assert.equal(points(line.d).length, 240);
  assert.equal(Math.min(...points(line.d).map(point => point[1])), 0.1, 'the one spike is kept, just under the top of a 0 to 1000 scale');
  assert.equal(reading(screen, 'Load'), undefined);
  screen.getByText('1000 values from 0 to 999. Lowest 0 at 0, highest 999 at 500. Rising: 0 first, 19 last.');
});

test('isSmooth: between two readings the curve never leaves their heights, however noisy the data', async () => {
  const screen = await renderPage(`
    import { LineChart } from '@liulidesign/charts/LineChart';
    const rows = Array.from({ length: 30 }, (_, index) => ({ t: String(index), v: Math.round(1800 + index * 46 + ((index * 53) % 17) * 38 - (index % 7 > 4 ? 520 : 0)) }));
    rows[11].v = rows[10].v; rows[20].v = 0; rows[21].v = 6000;
    export function render() { return <page><LineChart rows={rows} x="t" y="v" label="Visits" isSmooth hasScale={false} hasEndDot={false} /></page>; }
  `);
  const [line] = drawing(screen).paths;
  const segments = [...line.d.matchAll(/C([^C]+)/g)].map(match => points(match[1]));
  assert.equal(segments.length, 29);
  const bezier = (a, b, c, d, t) => (1 - t) ** 3 * a + 3 * (1 - t) ** 2 * t * b + 3 * (1 - t) * t ** 2 * c + t ** 3 * d;
  let from = points(line.d)[0];
  let turns = 0;
  for (const [c1, c2, to] of segments) {
    const [low, high] = [Math.min(from[1], to[1]), Math.max(from[1], to[1])];
    let before = from[0];
    for (let step = 1; step <= 40; step += 1) {
      const t = step / 40;
      const [x, y] = [0, 1].map(axis => bezier(from[axis], c1[axis], c2[axis], to[axis], t));
      assert.ok(y >= low - 0.011 && y <= high + 0.011, `at t=${t} between ${from} and ${to} the curve is at ${y}`);
      assert.ok(x > before, 'the curve never doubles back');
      before = x;
    }
    if (c1[1] === from[1] && c2[1] === to[1]) turns += 1;
    from = to;
  }
  assert.ok(turns >= 1, 'two equal readings in a row are joined by a level line');
});

test('states: one value is a dot; loading, empty and error replace the plot', async () => {
  const single = drawing(await renderExample('LineChart/LineChart', 'StateSingle')).paths;
  assert.ok(single.length > 0 && single.every(path => /^M50 20\.8h0\.01$/.test(path.d)), 'one value of 72.4 on a 62.5 to 75 scale');
  assert.equal((await renderExample('LineChart/LineChart', 'StateLoading')).getByTestId('loading').props.stateBusy, 1);
  (await renderExample('LineChart/LineChart', 'StateEmpty')).getByText('No weigh-ins yet');
  const error = await renderExample('LineChart/LineChart', 'StateError');
  assert.equal(error.getByText('Weigh-ins could not be loaded.').props.role, 'alert');
  assert.equal(error.queryAll(node => node.tag === 'vector').length, 1, 'the error mark alone: no plot');
});

test('a scale in the hundred thousands lays out with small numbers: no size taken from a data value', async () => {
  const screen = await renderPage(`
    import { LineChart } from '@liulidesign/charts/LineChart';
    const rows = [98400, 91200, 104800, 109300, 118700, 112400, 121900, 117600, 126300, 134800, 141200].map((revenue, index) => ({ month: String(index + 1), revenue }));
    export function render() { return <page><LineChart rows={rows} x="month" y="revenue" label="Revenue by month" hasArea isSmooth data-testid="year" /></page>; }
  `);
  const placed = screen.queryAll(node => /transform/.test(node.props.transition ?? '')).map(down);
  assert.ok(placed.length > 4, 'the scale and the end figure are placed by a share of the plot');
  assert.ok(placed.every(value => Number.isFinite(value) && value >= 0 && value <= 100), `shares of the plot, never past it: ${placed}`);
  const grows = screen.queryAll(node => node.tag !== 'vector' && node.props.grow !== undefined).map(node => node.props.grow);
  assert.ok(grows.every(value => value <= 1000), `flex shares are small: ${grows}`);
  assert.ok(JSON.stringify(screen.getByTestId('year')).includes('150000'), 'the scale tops out at a round 150000');
});

test('before the plot is laid out the line is hidden along its length; the fill waits in its own layer and fades in after it', async () => {
  const screen = await renderExample('LineChart/LineChart', 'Area');
  const [fill, line] = drawing(screen).paths;
  assert.deepEqual([line.key, line.pathLength, line.strokeDasharray, line.strokeDashoffset, line.strokeLinecap], ['visits-line', 1, [1], 1, 'butt']);
  const layer = () => drawings(screen).find(node => node.parent.props.position === 'absolute').parent;
  assert.equal(fill.fill, 'chart-1/16', 'the fill keeps its colour; its layer is clear');
  assert.equal(layer().props.opacity, 0);
  laidOut(screen);
  const [, after] = drawing(screen).paths;
  assert.deepEqual([after.strokeDashoffset, after.strokeLinecap], [0, 'round']);
  assert.equal(layer().props.opacity, undefined);
  assert.match(layer().props.transition ?? '', /opacity/);
  assert.match(lineVector(screen).props.transition ?? '', /"d"/, 'the line moves its paths');
});

test('a chart that comes back with the same rows shows whole from its first frame, without drawing in again', async () => {
  const page = linked => `import { LineChart } from ${JSON.stringify(`${linked}/src/LineChart/LineChart`)};
import { useLocal } from '@lingxia/view';
const rows = [{ day: 'Mon', visits: 3 }, { day: 'Tue', visits: 5 }, { day: 'Wed', visits: 4 }];
export function render() {
  const on = useLocal('on', true);
  return <page><button data-testid="toggle" onClick={() => on.set(!on.value)}><text>Toggle</text></button>{on.value && <LineChart label="Visits" rows={rows} x="day" y="visits" />}</page>;
}
`;
  const screen = laidOut(await renderPage(page));
  screen.press(screen.getByTestId('toggle'));
  screen.press(screen.getByTestId('toggle'));
  assert.equal(drawing(screen).paths[0].strokeDashoffset, 0, 'not laid out again yet, and still drawn');
});

/** A line chart whose rows change once, across a change of range: a test of what moves with the data. */
const changing = props => linked => `import { LineChart } from ${JSON.stringify(`${linked}/src/LineChart/LineChart`)};
import { useLocal } from '@lingxia/view';
const data = [[{ d: 'a', v: 3 }, { d: 'b', v: 7 }, { d: 'c', v: 5 }], [{ d: 'a', v: 4 }, { d: 'b', v: 2 }, { d: 'c', v: 9 }]];
export function render() {
  const step = useLocal('step', 0);
  return <page><button data-testid="next" onClick={() => step.set(1)}><text>Next</text></button><LineChart label="Visits" rows={data[step.value]} x="d" y="v" ${props} /></page>;
}
`;
/** A transition's timing, whatever it moves. */
const timing = node => { const { properties, ...rest } = JSON.parse(node.props.transition); return rest; };
/** A mover's place, exactly, as a percentage of the plot's height. */
const place = node => Number.parseFloat(JSON.parse(node.props.transform).translate[1]);
const parts = screen => { const [scale, plot, end] = lineVector(screen).parent.parent.children; return { scale, plot, end }; };

test('the end figure moves with the end of the line: the same start, end and timing, so level with it at every point of a change', async () => {
  const screen = laidOut(await renderPage(changing('hasEndLabel')));
  const read = () => [place(parts(screen).end.children[0]), points(drawing(screen).paths.find(path => path.key === 'v-line').d).at(-1)[1], textOf(parts(screen).end)];
  const [figureBefore, pointBefore, textBefore] = read();
  screen.press(screen.getByTestId('next'));
  const [figureAfter, pointAfter, textAfter] = read();
  assert.ok(Math.abs(figureBefore - pointBefore) < 0.01 && Math.abs(figureAfter - pointAfter) < 0.01, `${[figureBefore, pointBefore, figureAfter, pointAfter]}`);
  assert.notEqual(figureAfter, figureBefore, 'the end moved');
  assert.deepEqual([textBefore, textAfter], ['5', '9'], 'the figure says the new value at once; only its place travels');
  assert.deepEqual(timing(parts(screen).end.children[0]), timing(lineVector(screen)), 'one duration, easing and delay');
  const half = (from, to) => from + (to - from) / 2;
  assert.ok(Math.abs(half(figureBefore, figureAfter) - half(pointBefore, pointAfter)) < 0.01, 'half-way through, the figure is level with the point');
});

test('a tick is keyed by its value: one that stays when the range changes keeps its key and slides, on the lines\' timing, to its new place', async () => {
  const screen = laidOut(await renderPage(changing('')));
  const ticksNow = () => movers(parts(screen).scale).map(mover => ({ key: mover.key, text: textOf(mover), at: place(mover) }));
  const before = ticksNow();
  screen.press(screen.getByTestId('next'));
  const after = ticksNow();
  assert.ok(after.every(tick => tick.key === `tick-${tick.text}`), `${after.map(tick => tick.key)}`);
  const kept = after.filter(tick => before.some(old => old.key === tick.key));
  assert.ok(kept.length > 0, 'at least one tick stays');
  assert.ok(kept.some(tick => before.find(old => old.key === tick.key).at !== tick.at), 'and moves');
  const grid = parts(screen).plot.children.find(node => node.props.position === 'absolute' && movers(node).length > 0);
  assert.deepEqual(movers(grid).map(mover => [mover.key, place(mover)]), movers(parts(screen).scale).map(mover => [mover.key, place(mover)]), 'the hairlines move with their labels');
  assert.deepEqual(timing(movers(grid)[0]), timing(lineVector(screen)));
});

test('the chosen row\'s marker is keyed by its row: another row replaces it at once, new data under the same row moves it with the line', async () => {
  const page = chosen => `
    import { LineChart } from '@liulidesign/charts/LineChart';
    const rows = [{ d: 'a', v: 3 }, { d: 'b', v: 7 }, { d: 'c', v: 5 }];
    export function render() { return <page><LineChart label="Visits" rows={rows} x="d" y="v" value="${chosen}" onChange={() => {}} /></page>; }
  `;
  const marker = screen => screen.queryAll(node => node.tag === 'vector' && node.props.position === 'absolute')[0];
  const [b, c] = [await renderPage(page('b')), await renderPage(page('c'))];
  assert.deepEqual(JSON.parse(marker(b).props.vectorSource).paths.map(path => path.key), ['rule-1', 'v-1']);
  assert.deepEqual(JSON.parse(marker(c).props.vectorSource).paths.map(path => path.key), ['rule-2', 'v-2'], 'no key in common, so the renderer draws it at once');
  assert.match(marker(b).props.transition ?? '', /"d"/);
});

test('with reduced motion nothing that follows the data moves: lines, figure, ticks and marker take their new place at once', async () => {
  const screen = laidOut(await renderPage(changing('hasEndLabel value="b" onChange={() => {}}'), { env: defaultEnv({ reducedMotion: true }) }));
  const moving = screen.queryAll(node => node.props.transition !== undefined && /transform|"d"/.test(node.props.transition));
  assert.deepEqual(moving.map(node => node.tag), [], 'no transition of a place or a path');
  assert.ok(place(parts(screen).end.children[0]) >= 0, 'each still has its place');
});

/** Gives every measured text a size, as a host does: 7 pt a character, 16 pt high. */
const measured = screen => {
  // One at a time: each size re-renders the chart, which may replace the others.
  const done = new Set();
  for (;;) {
    const node = screen.queryAll(each => each.tag === 'text' && each.props.onLayout !== undefined && !done.has(`${each.props.onLayout}:${textOf(each)}`))[0];
    if (!node) return screen;
    done.add(`${node.props.onLayout}:${textOf(node)}`);
    screen.layout(node, { width: textOf(node).length * 7, height: 16 });
  }
};
const referencePaths = screen => screen.queryAll(node => node.tag === 'vector').flatMap(node => JSON.parse(node.props.vectorSource).paths.filter(path => path.key?.startsWith('reference-')).map(path => ({ ...path, vector: node })));
const referenceLabels = screen => screen.queryAll(node => node.tag === 'text' && node.props.insetInlineEnd !== undefined);

test('Goal: the range widens to the goal, drawn as a dashed hairline and named at the end with its value, on the line\'s timing', async () => {
  const screen = measured(laidOut(await renderExample('LineChart/LineChart', 'Goal')));
  const [path] = referencePaths(screen);
  const { d, stroke, strokeDasharray, vector } = path;
  const [, y] = d.match(/^M-?[\d.]+ (-?[\d.]+)H-?[\d.]+$/).map(Number);
  assert.deepEqual([stroke, strokeDasharray], ['text-muted', [4, 3]]);
  assert.ok(y > 0 && y <= 100, `the goal lies in the plot: ${y}`);
  assert.ok(screen.queryAll(node => node.tag === 'text' && textOf(node) === '70.0 kg' && !unread(node)).length === 0, 'the scale is hidden');
  const [label] = referenceLabels(screen);
  assert.equal(textOf(label), 'Goal 70.0 kg');
  assert.ok(unread(label), 'the label is decoration; the reading says it');
  const mover = label.parent.parent;
  assert.equal(JSON.parse(mover.props.transform).translate[1], `${y}%`, 'the label stands at the line');
  const timing = node => { const { durationMs, easing } = JSON.parse(node.props.transition); return JSON.stringify([durationMs, easing]); };
  assert.equal(timing(mover), timing(vector), 'label and line move together');
  assert.equal(timing(vector), timing(lineVector(screen)), 'and with the data');
  screen.getByText('Goal, 70.0 kg');
  assert.equal(screen.queryAll(node => node.tag === 'button' || node.props.focusable).length, 0, 'nothing takes a press or focus');
});

test('two references 2% apart: the second label moves below its line, a third slides along its line past the first', async () => {
  const screen = measured(laidOut(await renderPage(`
    import { LineChart } from '@liulidesign/charts/LineChart';
    const rows = [{ k: 'a', v: 0 }, { k: 'b', v: 100 }];
    export function render() { return <page><LineChart rows={rows} x="k" y="v" label="Chart" hasScale={false} references={[{ value: 50, label: 'Target' }, { value: 48, label: 'Average' }, { value: 49, label: 'Median' }]} /></page>; }
  `)));
  const sides = referenceLabels(screen).map(label => [textOf(label), label.props.insetBottom === 0 ? 'above' : 'below', label.props.insetInlineEnd]);
  assert.deepEqual(sides, [['Target 50', 'above', 0], ['Average 48', 'below', 0], ['Median 49', 'above', 'Target 50'.length * 7 + 2]]);
  for (const text of ['Target, 50', 'Average, 48', 'Median, 49']) screen.getByText(text);
});

test('a goal level with the last value: its label goes to the side of the line away from the latest point', async () => {
  const sides = async last => referenceLabels(measured(laidOut(await renderPage(`
    import { LineChart } from '@liulidesign/charts/LineChart';
    const rows = [{ k: 'a', v: 0 }, { k: 'b', v: 100 }, { k: 'c', v: ${last} }];
    export function render() { return <page><LineChart rows={rows} x="k" y="v" label="Chart" hasScale={false} references={[{ value: 50, label: 'Goal' }]} /></page>; }
  `)))).map(label => [textOf(label), label.props.insetBottom === 0 ? 'above' : 'below']);
  assert.deepEqual(await sides(56), [['Goal 50', 'below']], 'the latest point just above the line');
  assert.deepEqual(await sides(44), [['Goal 50', 'above']], 'the latest point just below it');
  assert.deepEqual(await sides(100), [['Goal 50', 'above']], 'far from it');
});

test('a goal the latest value reaches exactly: the label keeps its name and slides along the line, clear of the point', async () => {
  const screen = measured(laidOut(await renderPage(`
    import { LineChart } from '@liulidesign/charts/LineChart';
    const rows = [{ k: 'a', v: 0 }, { k: 'b', v: 100 }, { k: 'c', v: 50 }];
    export function render() { return <page><LineChart rows={rows} x="k" y="v" label="Chart" hasScale={false} references={[{ value: 50, label: 'Goal' }]} /></page>; }
  `)));
  const [label] = referenceLabels(screen);
  // The point sits 4 pt in from the end edge and is kept 6 pt clear all round; the label stops 2 pt short of that.
  assert.deepEqual([textOf(label), label.props.insetBottom === 0 ? 'above' : 'below', label.props.insetInlineEnd], ['Goal 50', 'above', 4 + 6 + 2]);
});
