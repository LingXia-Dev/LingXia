import assert from 'node:assert/strict';
import { test } from 'node:test';
import { fileURLToPath } from 'node:url';
import { probe } from '@liulidesign/core/internal/probe';

// Every example drawn with the theme's steps left as references: a pack moves the chrome. Data geometry stays in points.
const found = await probe(fileURLToPath(new URL('..', import.meta.url)));

/** Lengths the data sets, in points by design. */
const dataGeometry = new Set([
  // The figures' lane above each bar: the widest figure's measured line, at least 20 points.
  ...['Orders', 'Signed', 'Sizes', 'StateSelected', 'Target', 'Zone', 'Tones', 'Highlight'].map(example => `BarChart/${example}: height=20`),
  // A signed bar's slot leaves the figure lane on the side its bar grows away from.
  ...['Signed', 'Tones'].flatMap(example => [
    `BarChart/${example}: paddingTop=20`,
    `BarChart/${example}: paddingBottom=20`,
    // The marks' box clears the figure lane and the bar's rounded end, which is drawn into the path.
    `BarChart/${example}: insetBottom=23`,
    // The drawing reaches the rounded end back below the marks' box, as it paints only inside its own.
    `BarChart/${example}: insetBottom=-3`,
  ]),
  // A lying bar's line is as tall as its bars, which the drawing sets in points.
  ...['Horizontal', 'HorizontalStacked'].map(example => `BarChart/${example}: height=12`),
]);

test('every length drawn is a theme step, so a pack reaches it, except the data geometry', () => {
  assert.deepEqual(found.lengths.filter(line => !dataGeometry.has(line)), []);
  assert.deepEqual([...dataGeometry].filter(line => !found.lengths.includes(line)), [], 'an exemption no longer applies: take it off the list');
});

test('every colour drawn is a theme role', () => {
  assert.deepEqual(found.colours, []);
});

test('in high contrast every edge stands 3:1 off what it lies on', () => {
  assert.deepEqual(found.edges.map(({ example, edge, ratio }) => `${example}: ${edge}: ${ratio}`), []);
});
