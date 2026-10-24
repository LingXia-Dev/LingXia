import { test } from 'node:test';
import assert from 'node:assert/strict';
import { chartEmphasisColor } from '../lxapp/shared/chart-emphasis.ts';
import { liftColor, toHex } from '../../lingxia-showcase/lxapp/node_modules/zrender/lib/tool/color.js';

test('chart emphasis matches the React renderer channel lift and saturation', () => {
  const colors = ['#3B82F6','#8B5CF6','#10B981','#F59E0B','#EF4444','#06B6D4'];
  for (let channel = 0; channel < 256; channel++) {
    colors.push('#' + channel.toString(16).padStart(2, '0').repeat(3));
  }
  for (const color of colors) assert.equal(chartEmphasisColor(color), '#' + toHex(liftColor(color)));
});
