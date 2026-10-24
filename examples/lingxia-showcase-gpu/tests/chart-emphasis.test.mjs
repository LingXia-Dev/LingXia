import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { chartEmphasisColor } from '../lxapp/shared/chart-emphasis.ts';

const reference = JSON.parse(await readFile(new URL('./fixtures/chart-reference.json', import.meta.url), 'utf8'));

test('chart emphasis matches the React renderer channel lift and saturation', () => {
  for (const [color, lifted] of Object.entries(reference.emphasis)) assert.equal(chartEmphasisColor(color), lifted);
});
