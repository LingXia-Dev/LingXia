import { test } from 'node:test';
import assert from 'node:assert/strict';
import { textOf } from '@lingxia/view/testing';
import { renderExample } from '../testing/harness.mjs';

const hidden = node => Boolean(node.props.accessibilityHidden);

test('Budget: a group named by its label; the summary is read and not drawn; a custom plot fills the fixed-height box', async () => {
  const screen = await renderExample('Chart/Chart', 'Budget');
  const chart = screen.getByTestId('budget');
  assert.equal(chart.props.role, 'group');
  assert.equal(chart.props.accessibilityLabel, 'Budget for the quarter');
  const summary = screen.getByText('62% spent, 23% committed, 15% left.');
  assert.equal(summary.props.opacity, 0);
  assert.ok(!hidden(summary));
  const bar = screen.queryAll(node => node.props.flexDirection === 'row' && node.children.length === 3 && node.children[0].props.height === 24)[0];
  assert.deepEqual(bar.children.map(node => node.props.grow), [6200, 2300, 1500]);
  assert.equal(bar.parent.props.height, undefined, 'a fitted plot is as tall as its content');
  const legend = screen.queryAll(node => node.props.role === 'list')[0];
  assert.equal(legend.props.accessibilityLabel, 'Parts');
  assert.deepEqual(legend.children.map(textOf), ['Spent$6,200', 'Committed$2,300', 'Left$1,500']);
  assert.ok(legend.children.every(row => row.props.role === 'listitem' && row.props.minHeight === 36));
});

test('a title that repeats the label is hidden, so the name is read once', async () => {
  const screen = await renderExample('Chart/Chart', 'StateEmpty');
  assert.ok(!hidden(screen.getByText('Budget')), 'a title that differs from the label is read');
  const same = await renderExample('PieChart/PieChart', 'Merged');
  assert.ok(hidden(same.getByText('Readers by language')));
});

test('Legends: squares for marks, strokes with each line\'s dash; a pressable entry reports whether its series is shown', async () => {
  const screen = await renderExample('Chart/Chart', 'Legends');
  const squares = screen.getByTestId('squares');
  assert.deepEqual(squares.children.map(item => item.children[0].props.background), ['chart-1', 'chart-2', 'chart-3']);
  const entry = key => screen.getByTestId(`lines-${key}`);
  const stroke = node => JSON.parse(node.children[0].props.vectorSource).paths[0];
  assert.equal(stroke(entry('free')).strokeDasharray, undefined);
  assert.deepEqual(stroke(entry('team')).strokeDasharray, [6, 4]);
  assert.equal(entry('team').props.statePressed, 1);
  assert.equal(entry('pro').props.statePressed, 0, 'hidden to begin with');
  assert.equal(stroke(entry('pro')).stroke, 'text-faint');
  assert.equal(entry('pro').children[1].props.textDecorationLine, 'line-through');
  screen.press(entry('pro'));
  assert.equal(entry('pro').props.statePressed, 1);
  assert.equal(stroke(entry('pro')).stroke, 'chart-3');
  assert.equal(entry('team').parent.props.role, 'listitem');
});

test('Readout: a status line with the category, then each figure in tabular figures beside its series', async () => {
  const screen = await renderExample('Chart/Chart', 'Readout');
  const one = screen.getByTestId('one');
  assert.equal(one.props.role, 'status');
  assert.equal(one.props.minHeight, 18);
  assert.equal(textOf(one), 'Wednesday121');
  const two = screen.getByTestId('two');
  assert.equal(textOf(two), 'Q3Online$55kIn store$33k');
  assert.equal(screen.getByText('$55k').props.fontVariantNumeric, 'tabular-nums');
  assert.deepEqual(two.children.slice(1).map(value => value.children[0].props.background), ['chart-1', 'chart-2']);
});

test('states: loading is busy with a hidden skeleton; empty shows its words; an error is an alert; none draws a legend', async () => {
  const loading = (await renderExample('Chart/Chart', 'StateLoading')).getByTestId('loading');
  assert.equal(loading.props.stateBusy, 1);
  assert.match(JSON.stringify(loading), /skeleton-glint/);
  const empty = await renderExample('Chart/Chart', 'StateEmpty');
  assert.equal(empty.getByText('No budget set').props.color, 'text-muted');
  assert.equal(empty.getByText('No budget set').parent.props.alignSafe, 1, 'a message taller than the plot starts at its top');
  const error = await renderExample('Chart/Chart', 'StateError');
  assert.equal(error.getByText('The budget could not be loaded').props.role, 'alert');
  for (const screen of [empty, error]) assert.equal(screen.queryAll(node => node.props.role === 'list').length, 0);
});

test('Spending: a sixth, seventh and eighth series take hues', async () => {
  const screen = await renderExample('Chart/Chart', 'Spending');
  assert.deepEqual(['health', 'leisure', 'clothes', 'gifts'].map(key => screen.getByTestId(`part-${key}`).props.background), ['chart-5', 'hue-3', 'hue-5', 'hue-4']);
});
