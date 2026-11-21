import { test } from 'node:test';
import assert from 'node:assert/strict';
import { chartScale } from '../lxapp/shared/chart-scale.ts';

test('quarterly sales use readable ticks without changing the data range', () => {
  assert.deepEqual(chartScale([24, 38, 31, 52]), {
    min: 0, max: 60, ticks: [60, 50, 40, 30, 20, 10, 0],
  });
});

test('signed, decimal, constant and bounded extremes have finite containing scales', () => {
  for (const values of [[0, 0], [7, 7], [-52, -24], [-10, 20], [0.03, 0.08], [0.1, 0.3], [-0.3, 0.1], [-1e9, 1e9], [Number.MIN_VALUE]]) {
    const scale = chartScale(values);
    assert.ok(scale.min <= 0 && scale.max >= 0 && scale.max > scale.min);
    assert.ok(values.every(value => scale.min <= value && value <= scale.max));
    assert.equal(scale.ticks[0], scale.max);
    assert.equal(scale.ticks.at(-1), scale.min);
    assert.ok(scale.ticks.every((value, i) => Number.isFinite(value) && (!i || value < scale.ticks[i - 1])));
    assert.ok(scale.ticks.length <= 17);
  }
  assert.deepEqual(chartScale([0.03, 0.08]).ticks, [0.08, 0.06, 0.04, 0.02, 0]);
});
