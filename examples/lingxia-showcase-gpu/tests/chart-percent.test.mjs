import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { chartPercentages } from '../lxapp/shared/chart-percent.ts';

const reference = JSON.parse(await readFile(new URL('./fixtures/chart-reference.json', import.meta.url), 'utf8'));

test('pie percentages match the React chart precision and remainder allocation', () => {
  for (const { values, seats } of reference.percent) assert.deepEqual(chartPercentages(values), seats.length ? seats : values.map(() => 0));
  assert.deepEqual(chartPercentages([1, 1, 1]), [33.34, 33.33, 33.33]);
});
