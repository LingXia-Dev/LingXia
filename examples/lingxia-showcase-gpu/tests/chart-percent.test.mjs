import { test } from 'node:test';
import assert from 'node:assert/strict';
import { chartPercentages } from '../lxapp/shared/chart-percent.ts';
import { getPercentSeats } from '../../lingxia-showcase/lxapp/node_modules/echarts/lib/util/number.js';

test('pie percentages match the React chart precision and remainder allocation', () => {
  const cases = [[24,38,31,52],[24,31,52],[1,1,1],[0,0],[0,1,0],[Number.MIN_VALUE,Number.MIN_VALUE],[1e9,1,3],[]];
  let seed = 23;
  for(let count=1;count<=128;count++) cases.push(Array.from({length:count},()=>{seed=(Math.imul(seed,1664525)+1013904223)>>>0;return seed%10000;}));
  for(const values of cases){
    const expected = getPercentSeats(values,2);
    assert.deepEqual(chartPercentages(values), expected.length ? expected : values.map(()=>0));
  }
  assert.deepEqual(chartPercentages([1,1,1]),[33.34,33.33,33.33]);
});
