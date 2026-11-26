import { test } from 'node:test';
import assert from 'node:assert/strict';
import { renderExample, renderPage } from '../testing/harness.mjs';

const drawing = node => JSON.parse(node.children[0].props.vectorSource);
const page = (props, values) => `
  import { Sparkline } from '@liulidesign/charts/Sparkline';
  const values = ${values};
  export function render() { return <page><Sparkline values={values} label="Readings" data-testid="spark" ${props} /></page>; }
`;
/** Every coordinate pair in a path, in order. */
const points = d => [...d.matchAll(/(-?[\d.]+) (-?[\d.]+)/g)].map(match => [Number(match[1]), Number(match[2])]);

test('Trend: one image named by its label, the drawing hidden; a line across the box with the latest value dotted', async () => {
  const screen = await renderExample('Sparkline/Sparkline', 'Trend');
  const spark = screen.getByTestId('trend');
  assert.equal(spark.props.role, 'img');
  assert.equal(spark.props.accessibilityLabel, 'Revenue, last 7 days: up from 12k to 24k');
  assert.equal(spark.children[0].props.accessibilityHidden, 1);
  const { viewBox, fit, paths } = drawing(spark);
  assert.deepEqual(viewBox, [-3.333, -11.538, 106.667, 123.077], 'until measured, 96 by 32 pt: the viewBox reaches 3 pt past the data, so the end dot is whole');
  screen.layout(spark, { width: 126, height: 46 });
  assert.deepEqual(drawing(screen.getByTestId('trend')).viewBox, [-2.5, -7.5, 105, 115], '3 of 120 pt across, 3 of 40 pt up and down');
  assert.equal(fit, 'fill');
  const [line, dot] = paths;
  assert.deepEqual(points(line.d)[0], [0, 100], 'the lowest value sits on the floor: a line fits its data');
  assert.deepEqual(points(line.d).at(-1), [100, 0]);
  assert.equal(line.vectorEffect, 'non-scaling-stroke');
  assert.equal(dot.d, 'M100 0h0.01');
  assert.ok(dot.strokeWidth > line.strokeWidth);
  assert.equal(dot.strokeLinecap, 'round');
});

test('Variants: an area is measured from zero and filled under its line; bars stand on the zero line', async () => {
  const screen = await renderExample('Sparkline/Sparkline', 'Variants');
  const line = drawing(screen.getByTestId('line')).paths;
  assert.equal(line.length, 1);
  assert.equal(Math.max(...points(line[0].d).map(point => point[1])), 100);
  const [fill, stroke] = drawing(screen.getByTestId('area')).paths;
  assert.equal(fill.fill, 'chart-1/16');
  assert.match(fill.d, /L100 100L0 100Z$/);
  assert.equal(stroke.stroke, 'chart-1');
  assert.ok(Math.max(...points(stroke.d).map(point => point[1])) < 70, '38 of 108 is well above the floor when zero is in range');
  const [bars] = drawing(screen.getByTestId('bar')).paths;
  assert.equal(bars.fill, 'chart-1');
  const rects = bars.d.split('Z').filter(Boolean);
  assert.equal(rects.length, 10);
  // Values 6 and -4 with a range of -7..11: the zero line is 11/18 down, and both bars touch it.
  const zero = 61.11;
  assert.match(rects[0], /^M1\.4 27\.78h7\.2v33\.33/);
  assert.match(rects[2], new RegExp(`^M21\\.4 ${zero}h7\\.2v22\\.22`));
});

test('Smooth: a curve through every value that never leaves the range of its two ends', async () => {
  const screen = await renderExample('Sparkline/Sparkline', 'Smooth');
  const stroke = drawing(screen.getByTestId('smooth')).paths[1];
  const segments = [...stroke.d.matchAll(/C([^C]+)/g)].map(match => points(match[1]));
  assert.equal(segments.length, 6);
  let from = points(stroke.d)[0];
  for (const [c1, c2, to] of segments) {
    const [low, high] = [Math.min(from[1], to[1]), Math.max(from[1], to[1])];
    for (const control of [c1, c2]) assert.ok(control[1] >= low - 0.01 && control[1] <= high + 0.01, `control ${control} leaves ${low}..${high}`);
    from = to;
  }
});

test('Tones: the tone is the stroke role', async () => {
  const screen = await renderExample('Sparkline/Sparkline', 'Tones');
  assert.equal(drawing(screen.getByTestId('positive')).paths[0].stroke, 'success-mark');
  assert.equal(drawing(screen.getByTestId('negative')).paths[0].stroke, 'danger-mark');
});

test('Sizes: 24, 32 and 48 tall, as wide as the container', async () => {
  const screen = await renderExample('Sparkline/Sparkline', 'Sizes');
  assert.equal(screen.getByTestId('small').props.height, 24);
  assert.equal(screen.getByTestId('large').props.height, 48);
  assert.equal(screen.getByTestId('large').props.alignSelf, 'stretch');
  assert.equal(screen.getByTestId('large').children[0].props.widthPercent, 100, 'a vector keeps its own 24 pt until sized');
});

test('Gaps: a missing reading breaks the line, and a reading between two gaps is a dot', async () => {
  const screen = await renderExample('Sparkline/Sparkline', 'Gaps');
  const [line, alone] = drawing(screen.getByTestId('gaps')).paths;
  assert.equal(line.d.split('M').filter(Boolean).length, 3);
  assert.equal(alone.d, 'M63.64 30.77h0.01');
  assert.equal(alone.strokeLinecap, 'round');
});

test('states: no values keeps the named box and draws nothing; one value is a dot; equal values are a level line', async () => {
  const empty = (await renderExample('Sparkline/Sparkline', 'StateEmpty')).getByTestId('empty');
  assert.equal(empty.props.accessibilityLabel, 'No readings yet');
  assert.equal(empty.props.height, 32);
  assert.equal(empty.children.length, 0);
  const single = drawing((await renderExample('Sparkline/Sparkline', 'StateSingle')).getByTestId('single')).paths;
  assert.deepEqual(single.map(path => path.d), ['M50 50h0.01']);
  const flat = drawing((await renderExample('Sparkline/Sparkline', 'StateFlat')).getByTestId('flat')).paths[0];
  assert.deepEqual([...new Set(points(flat.d).map(point => point[1]))], [50]);
});

test('InRows: each row reads its sparkline by its own label', async () => {
  const screen = await renderExample('Sparkline/Sparkline', 'InRows');
  const images = screen.queryAll(node => node.props.role === 'img');
  assert.deepEqual(images.map(image => image.props.accessibilityLabel.split(',')[0]), ['Lisbon', 'Oslo', 'Nairobi']);
  assert.ok(images.every(image => image.props.width === 80 && image.props.height === 24));
});

test('domainMin and domainMax widen the range and never cut the data', async () => {
  const wide = drawing((await renderPage(page('domainMin={0} domainMax={40}', '[10, 20, 30]'))).getByTestId('spark')).paths[0];
  assert.deepEqual(points(wide.d).map(point => point[1]), [75, 50, 25]);
  const narrow = drawing((await renderPage(page('domainMin={15} domainMax={25}', '[10, 20, 30]'))).getByTestId('spark')).paths[0];
  assert.deepEqual(points(narrow.d).map(point => point[1]), [100, 50, 0]);
});

test('past 240 values, the series is reduced to 240 points that keep its peaks, dips and gaps', async () => {
  const series = 'Array.from({ length: 2400 }, (_, index) => (index === 1203 ? 90 : index === 1807 ? -40 : index >= 600 && index < 640 ? NaN : index % 10))';
  const screen = await renderPage(page('isSmooth', series));
  const [line] = drawing(screen.getByTestId('spark')).paths;
  assert.equal(line.d.split('M').filter(Boolean).length, 2, 'the forty missing readings are still one gap');
  assert.ok(line.d.match(/C/g).length <= 238);
  const heights = points(line.d).map(point => point[1]);
  assert.equal(Math.min(...heights), 0, 'the one reading of 90 is drawn at the top');
  assert.equal(Math.max(...heights), 100, 'the one reading of -40 is drawn at the bottom');
  const source = screen.getByTestId('spark').children[0].props.vectorSource;
  assert.ok(source.length < 16384, `${source.length} bytes`);
});

test('bars: a value that is not finite leaves its slot empty', async () => {
  const [bars] = drawing((await renderPage(page('variant="bar"', '[4, NaN, 2]'))).getByTestId('spark')).paths;
  assert.equal(bars.d.split('Z').filter(Boolean).length, 2);
});

test('an area over negative values closes at the zero line, not at the bottom of the box', async () => {
  const screen = await renderPage(page('variant="area"', '[-2, 1, -1]'));
  const [fill] = drawing(screen.getByTestId('spark')).paths;
  assert.match(fill.d, /L100 33\.33L0 33\.33Z$/, 'zero is a third of the way down a range from -2 to 1');
});
