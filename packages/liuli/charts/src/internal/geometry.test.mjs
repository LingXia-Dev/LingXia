import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { transform } from 'esbuild';

const { code } = await transform(readFileSync(new URL('./geometry.ts', import.meta.url), 'utf8'), { loader: 'ts', format: 'esm' });
const { niceTicks, shares, domain } = await import(`data:text/javascript;base64,${Buffer.from(code).toString('base64')}`);

// What a layout accepts as a flex number: whole, finite, never negative, and small.
const layoutNumber = value => Number.isInteger(value) && value >= 0 && value <= 16384;

// The gallery dashboard's three periods, and data of every awkward kind.
const week = [4120, 3980, 4460, 4710, 5240, 6130, 5380];
const month = [3650, 3820, 4010, 3760, 4240, 4980, 4420, 3890, 4050, 4310, 4120, 4690, 5520, 4870, 4020, 4180, 4460, 4290, 4830, 5710, 5060, 4120, 3980, 4460, 4710, 5240, 6130, 5380, 4350, 4520];
const year = [98400, 91200, 104800, 109300, 118700, 112400, 121900, 117600, 126300, 134800, 141200];
const sets = {
  week, month, year,
  flat: [5, 5, 5], zeros: [0, 0, 0], one: [72.4], none: [],
  negatives: [-12, -3, -40, -7], signed: [-250, 100, 40, -80, 310],
  huge: [2.4e12, 9.9e12, 5e12], tiny: [0.00012, 0.00019, 0.00015], closeTogether: [1000000.1, 1000000.4],
  weight: [72.4, 72.1, 71.8, 71.6, 71.2, 70.9, 70.5], decimalsOnAStep: [0.1, 0.2, 0.3], powersOfTen: [10, 100, 1000],
};
// A seeded spread on top, so the sweep is wide and the same on every run.
let seed = 7;
const random = () => { seed = (seed * 16807) % 2147483647; return seed / 2147483647; };
for (let index = 0; index < 400; index += 1) {
  const scale = 10 ** Math.floor(random() * 16 - 6);
  const base = (random() - 0.5) * scale * 4;
  sets[`random ${index}`] = Array.from({ length: 1 + Math.floor(random() * 12) }, () => base + random() * scale);
}

test('niceTicks: every tick is inside the range, and the gaps are layout numbers that add up to it', () => {
  for (const [name, values] of Object.entries(sets)) {
    for (const hasZero of [false, true]) {
      for (const target of [2, 4]) {
        const fitted = domain(values, hasZero);
        const { ticks, range, gaps } = niceTicks(fitted, target);
        const where = `${name}, ${hasZero ? 'from zero' : 'fitted'}, ${target} ticks: ticks ${ticks} in ${range}, gaps ${gaps}`;
        assert.ok(range[0] <= fitted[0] && range[1] >= fitted[1], `the range holds the data: ${where}`);
        assert.ok(ticks.length >= 1 && ticks.every(tick => Number.isFinite(tick) && tick >= range[0] && tick <= range[1]), `ticks inside the range: ${where}`);
        assert.ok(ticks.every((tick, at) => at === 0 || tick > ticks[at - 1]), `ticks rise: ${where}`);
        assert.equal(gaps.length, ticks.length + 1, `a gap above, between and under: ${where}`);
        assert.ok(gaps.every(layoutNumber), `gaps are layout numbers: ${where}`);
        assert.ok(gaps.reduce((sum, gap) => sum + gap, 0) > 0, `the gaps hold the plot open: ${where}`);
        // The gaps place each tick where its value is.
        const total = gaps.reduce((sum, gap) => sum + gap, 0);
        if (range[1] > range[0]) ticks.forEach((tick, at) => {
          const under = gaps.slice(gaps.length - 1 - at).reduce((sum, gap) => sum + gap, 0);
          assert.ok(Math.abs(under / total - (tick - range[0]) / (range[1] - range[0])) < 1e-6, `tick ${tick} sits at its value: ${where}`);
        });
      }
    }
  }
});

test('niceTicks: a range that is not a number, or upside down, still gives a scale', () => {
  for (const range of [[NaN, 5], [0, Infinity], [9, 2], [-Infinity, NaN]]) {
    const { ticks, gaps } = niceTicks(range, 4);
    assert.ok(ticks.every(Number.isFinite) && gaps.every(layoutNumber), `${range}: ${ticks} / ${gaps}`);
  }
});

test('shares: any lengths become whole parts of 1000, never negative, never all zero', () => {
  for (const lengths of [[50000, 100000], [0, 0], [-4, 9], [NaN, 3], [1e-9, 2e-9], [Infinity, 1], [0.3, 0.7, 0], [141200, 8800]]) {
    const parts = shares(...lengths);
    assert.equal(parts.length, lengths.length);
    assert.ok(parts.every(layoutNumber) && parts.some(part => part > 0), `${lengths} gives ${parts}`);
  }
  assert.deepEqual(shares(141200, 8800), [941, 59]);
});
