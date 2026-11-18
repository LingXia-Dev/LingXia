import { test } from 'node:test';
import assert from 'node:assert/strict';
import { textOf } from '@lingxia/view/testing';
import { renderExample, renderPage } from '../testing/harness.mjs';

const ring = screen => screen.queryAll(node => node.tag === 'vector' && node.props.gridColumn === '1')[0];
const slices = screen => JSON.parse(ring(screen).props.vectorSource).paths;
const centre = screen => textOf(screen.queryAll(node => node.props.role === 'status')[0]);
/** The radius of a path's first arc. */
const rim = path => Number(/A([\d.]+) /.exec(path.d)[1]);

test('Spending: a ring of gapped slices in series colours, the total in the hole, and the legend as the reading', async () => {
  const screen = await renderExample('PieChart/PieChart', 'Spending');
  assert.equal(screen.getByTestId('spending').props.accessibilityLabel, 'Spending by category');
  const paths = slices(screen);
  assert.deepEqual(paths.map(path => path.fill), ['chart-1', 'chart-2', 'chart-3', 'chart-4', 'chart-5']);
  assert.ok(paths.every(path => rim(path) === 46 && /A29 29 /.test(path.d)), 'every slice runs from the hole to the rim');
  const start = d => /^M([\d.]+) ([\d.]+)/.exec(d).slice(1).map(Number);
  assert.deepEqual(start(paths[0].d), [50.8, 4.01], 'the first slice starts at the top, moved in by half the gap');
  assert.match(paths[4].d, /A46 46 0 0 1 49\.2 4\.01/, 'and the last ends half a gap short of it');
  assert.deepEqual([ring(screen).props.width, ring(screen).props.height], [176, 176], 'the ring is as tall as the medium plot');
  assert.equal(centre(screen), 'Total$2,860');
  screen.getByText('5 parts, $2,860 in all. Largest: Rent, $1,450, 51%. Smallest: Leisure, $180, 6%.');
  const legend = screen.getByTestId('spending-legend');
  assert.equal(legend.props.role, 'list');
  assert.deepEqual(legend.children.map(textOf), ['Rent$1,45051%', 'Food$62022%', 'Transport$2107%', 'Leisure$1806%', 'Savings$40014%']);
  assert.equal(screen.getByText('$1,450').props.fontVariantNumeric, 'tabular-nums');
});

test('Spending: a legend row chooses its part: the slice grows, the others step back, the hole and the row say which', async () => {
  const screen = await renderExample('PieChart/PieChart', 'Spending');
  const row = key => screen.getByTestId(`spending-legend-${key}`);
  assert.equal(row('Food').tag, 'button');
  assert.equal(row('Food').props.minHeight, 36);
  assert.equal(row('Food').props.statePressed, 0);
  screen.press(row('Food'));
  assert.equal(row('Food').props.statePressed, 1);
  assert.equal(row('Food').props.background, 'text/6');
  const paths = slices(screen);
  assert.deepEqual(paths.map(path => path.fill), ['chart-1/35', 'chart-2', 'chart-3/35', 'chart-4/35', 'chart-5/35']);
  assert.deepEqual(paths.map(rim), [46, 50, 46, 46, 46]);
  assert.equal(centre(screen), 'Food$620');
  screen.press(row('Food'));
  assert.equal(row('Food').props.statePressed, 0);
  assert.equal(centre(screen), 'Total$2,860');
  // The smallest slice, 6% of the ring, is chosen through a row as tall and wide as any other.
  assert.deepEqual([row('Leisure').props.minHeight, row('Leisure').parent.parent.props.alignSelf], [36, 'stretch']);
  screen.press(row('Leisure'));
  assert.equal(centre(screen), 'Leisure$180');
  assert.deepEqual(slices(screen).map(rim), [46, 46, 46, 50, 46]);
});

test('Solid: every slice is cut back from its neighbours, so the gaps show whatever surface the chart sits on', async () => {
  const screen = await renderExample('PieChart/PieChart', 'Solid');
  const paths = slices(screen);
  const start = d => /^M([\d.]+) ([\d.]+)/.exec(d).slice(1).map(Number);
  const end = d => /A46 46 0 [01] 1 ([\d.]+) ([\d.]+)/.exec(d).slice(1).map(Number);
  for (const [index, path] of paths.entries()) {
    const next = paths[(index + 1) % paths.length];
    const gap = Math.hypot(start(next.d)[0] - end(path.d)[0], start(next.d)[1] - end(path.d)[1]);
    assert.ok(Math.abs(gap - 1.6) < 0.05, `the gap after slice ${index} is ${gap} of 100 units`);
    assert.ok(!/surface|canvas|#/.test(JSON.stringify(path)), 'no slice paints a background colour');
  }
});

test('Solid: slices meet short of the centre, no hole text, and the legend rows are not buttons without onChange', async () => {
  const screen = await renderExample('PieChart/PieChart', 'Solid');
  const paths = slices(screen);
  assert.equal(paths.length, 4);
  assert.ok(paths.every(path => !/A29 /.test(path.d) && /L[\d.]+ [\d.]+Z$/.test(path.d)));
  assert.equal(screen.queryAll(node => node.props.role === 'status').length, 0);
  assert.equal(screen.queryAll(node => node.tag === 'button').length, 0);
  assert.equal(screen.getByTestId('storage-legend').children.length, 4);
});

test('Merged: the four largest keep a slice and the rest share one neutral slice, named by the app', async () => {
  const screen = await renderExample('PieChart/PieChart', 'Merged');
  assert.deepEqual(slices(screen).map(path => path.fill), ['chart-1', 'chart-2', 'chart-3', 'chart-4', 'line-strong']);
  const rows = screen.getByTestId('languages-legend').children.map(textOf);
  assert.equal(rows.length, 5);
  assert.equal(rows[4], 'Other1,63016%');
  assert.equal(centre(screen), 'Readers10,210');
});

test('parts without a positive amount are left out; without otherLabel every part is drawn', async () => {
  const screen = await renderPage(`
    import { PieChart } from '@liulidesign/charts/PieChart';
    const rows = [{ k: 'a', v: 3 }, { k: 'b', v: 0 }, { k: 'c', v: null }, { k: 'd', v: 1 }, { k: 'e', v: 1 }, { k: 'f', v: 1 }, { k: 'g', v: 1 }, { k: 'h', v: 1 }];
    export function render() { return <page><PieChart rows={rows} nameField="k" valueField="v" label="Parts" data-testid="pie" /></page>; }
  `);
  assert.equal(slices(screen).length, 6);
  assert.deepEqual(screen.getByTestId('pie-legend').children.map(textOf), ['a3', 'd1', 'e1', 'f1', 'g1', 'h1']);
  assert.deepEqual(slices(screen).map(path => path.fill).slice(4), ['chart-5', 'chart-6'], 'a sixth part takes the sixth chart colour');
});

test('states: a part chosen from outside; one part is a whole ring; loading, empty and error replace the plot', async () => {
  const selected = await renderExample('PieChart/PieChart', 'StateSelected');
  assert.equal(centre(selected), 'Apps21 GB');
  assert.deepEqual(slices(selected).map(rim), [46, 50, 46, 46]);
  const whole = slices(await renderExample('PieChart/PieChart', 'StateWhole'));
  assert.deepEqual([whole.length, whole[0].fill, whole[0].stroke, whole[0].strokeWidth], [1, 'none', 'chart-1', 17]);
  assert.equal((await renderExample('PieChart/PieChart', 'StateLoading')).getByTestId('loading').props.stateBusy, 1);
  (await renderExample('PieChart/PieChart', 'StateEmpty')).getByText('Nothing stored yet');
  const error = await renderExample('PieChart/PieChart', 'StateError');
  assert.equal(error.getByText('Storage could not be read.').props.role, 'alert');
  assert.equal(error.queryAll(node => node.tag === 'vector').length, 1, 'the error mark alone: no plot');
});
