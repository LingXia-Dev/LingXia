import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { transform } from 'esbuild';

const { code } = await transform(readFileSync(new URL('./geometry.ts', import.meta.url), 'utf8'), { loader: 'ts', format: 'esm' });
const { niceTicks, domain, placeReferences, knockOuts } = await import(`data:text/javascript;base64,${Buffer.from(code).toString('base64')}`);

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


test('placeReferences: a label above its line, the next one below when two lines are 2% apart, a third slid along its line past the first', () => {
  const plot = { width: 300, height: 200 };
  const label = { full: [80, 80, 80], short: [30, 30, 30], height: 16 };
  // Lines at 50%, 52% and 51% of the plot.
  const places = placeReferences([100, 104, 102], plot, label, [], 2);
  assert.deepEqual(places, [{ side: 'above', inset: 0, isShort: false }, { side: 'below', inset: 0, isShort: false }, { side: 'above', inset: 82, isShort: false }]);
  // In a plot too narrow to slide the name past the others, the value alone: slid, or at the end edge when nothing fits.
  assert.deepEqual(placeReferences([100, 104, 102], { width: 140, height: 200 }, label, [], 2)[2], { side: 'above', inset: 82, isShort: true });
  assert.deepEqual(placeReferences([100, 104, 102], { width: 100, height: 200 }, label, [], 2)[2], { side: 'above', inset: 0, isShort: true });
});

test('placeReferences: never past the plot\'s edges; a line at the top puts its label below, at the foot above', () => {
  const plot = { width: 300, height: 200 };
  const label = { full: [80, 80], short: [30, 30], height: 16 };
  assert.deepEqual(placeReferences([4, 198], plot, label, [], 2), [{ side: 'below', inset: 0, isShort: false }, { side: 'above', inset: 0, isShort: false }]);
});

test('placeReferences: a figure at the end edge sends the label to the other side; one clear of it across does not', () => {
  const plot = { width: 300, height: 200 };
  const label = { full: [80], short: [30], height: 16 };
  const figure = { start: 270, end: 290, top: 70, bottom: 98 };
  assert.deepEqual(placeReferences([100], plot, label, [figure], 2), [{ side: 'below', inset: 0, isShort: false }]);
  assert.deepEqual(placeReferences([100], plot, label, [{ ...figure, start: 20, end: 40 }], 2), [{ side: 'above', inset: 0, isShort: false }]);
});

test('placeReferences: blocked on both sides at the end edge, the name slides along its line past the obstacle; the value alone only where it cannot', () => {
  const label = { full: [120], short: [30], height: 16 };
  // A point on the line with room around it, as a line's latest value level with a goal.
  const point = { start: 284, end: 296, top: 94, bottom: 106 };
  assert.deepEqual(placeReferences([100], { width: 300, height: 200 }, label, [point], 2), [{ side: 'above', inset: 18, isShort: false }]);
  // Figures above and below, under the name's width but clear of the value's: too far in to slide the name past in a narrow plot.
  const taken = [{ start: 110, end: 130, top: 70, bottom: 98 }, { start: 110, end: 130, top: 101, bottom: 130 }];
  assert.deepEqual(placeReferences([100], { width: 220, height: 200 }, label, taken, 2), [{ side: 'above', inset: 0, isShort: true }]);
});

test('knockOuts: a line breaks only at the labels its stroke touches, 2 pt wider on each side, overlapping gaps as one', () => {
  const labels = [
    { start: 60, end: 80, top: 40, bottom: 56 },
    { start: 10, end: 31, top: 40, bottom: 56 },
    { start: 30, end: 50, top: 50, bottom: 66 },
    { start: 100, end: 120, top: 70, bottom: 86 },
  ];
  assert.deepEqual(knockOuts(52, labels, 2), [[8, 52], [58, 82]], 'in start order; the two near the start joined');
  assert.deepEqual(knockOuts(39.6, labels, 2), [[8, 33], [58, 82]], 'a hairline half a point above a box still touches it');
  assert.deepEqual(knockOuts(39.4, labels, 2), []);
  assert.deepEqual(knockOuts(90, labels, 2), []);
});
