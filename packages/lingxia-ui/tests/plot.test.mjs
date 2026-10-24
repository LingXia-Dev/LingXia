import { test } from 'node:test';
import assert from 'node:assert/strict';
import { h, describe, reset, dispatch, prepare, discard, useLocal } from '../src/runtime.js';
import { audit } from '../compiler.mjs';
const view = { data: {}, env: {} };
const plot = overrides => h('plot', { accessibilityLabel: 'Revenue', kind: 'bar', values: [10, 20], colors: ['#435DD9'], style: { height: 180 }, ...overrides });

test('line marker emphasis has a bounded append-only property', () => {
  for (const markerEmphasisScale of [1,1.1,4]) {
    reset();
    const wire=describe(()=>plot({kind:'line',markerEmphasisScale}),view);
    assert.equal(wire[wire.indexOf(94)+1],markerEmphasisScale);
  }
  for (const props of [{kind:'bar',markerEmphasisScale:1.1},{kind:'pie',markerEmphasisScale:1.1},
    ...[0.9,4.1,NaN,Infinity,'1.1',null].map(markerEmphasisScale=>({kind:'line',markerEmphasisScale}))]) {
    reset();assert.throws(()=>describe(()=>plot(props),view),/UI_PLOT/);
  }
});

test('plot uses append-only wire fields with bounded JSON arrays', () => {
  reset();
  const wire = describe(() => plot({ kind: 'pie' }), view);
  assert.equal(wire[0], 18);
  assert.ok(wire.includes('[10,20]'));
  assert.ok(wire.includes('["#435DD9"]'));
  assert.doesNotThrow(() => audit('const view = <plot kind="line" values={[1,2]} colors={["#123456"]} />;', 'plot.ui.tsx'));
});

test('invalid plot data, palette and children fail before commit', () => {
  for (const props of [
    { accessibilityLabel: '' }, { accessibilityLabel: '   ' },
    { accessibilityLabel: 'a\0b' }, { accessibilityLabel: 'a'.repeat(1025) },
    { kind: 'scatter' }, { values: [] }, { values: Array(129).fill(1) },
    { values: [NaN] }, { values: [1e10] }, { kind: 'pie', values: [-1] },
    { colors: [] }, { colors: ['url(file)'] }, { colors: Array(17).fill('#123456') },
    { kind: 'line', smooth: 1 }, { kind: 'line', area: 'yes' },
    { smooth: true }, { kind: 'pie', area: true },
    { onSelect: true }, { selectedIndex: 2 }, { selectedIndex: -2 },
    { selectedIndex: 0.5 }, { selectedIndex: '0' },
  ]) {
    reset(); assert.throws(() => describe(() => plot(props), view), /UI_PLOT/);
  }
  reset();
  assert.throws(() => describe(() => h('plot', { accessibilityLabel: 'Revenue', kind: 'bar', values: [1], colors: ['#123456'] }, 'child'), view), /UI_CHILD/);
});

test('selection delivers bounded numeric indices only from committed handles', () => {
  reset();
  let calls = [];
  const render = () => {
    const selected = useLocal('selected', -1);
    return plot({ selectedIndex: selected.value, onSelect: index => { calls.push(index); selected.set(index); } });
  };
  const fields = wire => new Map(Array.from({length:wire[5]}, (_, i) => wire.slice(6+i*2, 8+i*2)));
  let output = fields(describe(render, view));
  const old = output.get(56);
  assert.equal(output.get(57), -1);
  for (const value of [undefined, '1', true, NaN, Infinity, -2, 2, 0.5]) dispatch(old, value);
  assert.deepEqual(calls, []);
  assert.equal(dispatch(old, 1), true);
  output = fields(describe(render, view));
  assert.equal(output.get(57), 1);
  assert.equal(dispatch(old, 0), false);
  const pending = fields(prepare(render, view)).get(56);
  discard();
  assert.equal(dispatch(pending, 0), false);
  assert.equal(dispatch(output.get(56), 0), true);
  assert.deepEqual(calls, [1, 0]);
  output = fields(describe(render, view));
  assert.equal(dispatch(output.get(56), -1), true);
  output = fields(describe(render, view));
  assert.equal(output.get(57), -1);
  assert.deepEqual(calls, [1, 0, -1]);
});

test('smooth area fields are optional and share the expanded geometry budget', () => {
  reset();
  const wire = describe(() => plot({ kind: 'line', smooth: true, area: true }), view);
  const fields = new Map();
  for (let i = 0; i < wire[5]; i++) fields.set(wire[6 + i * 2], wire[7 + i * 2]);
  assert.equal(fields.get(54), 1); assert.equal(fields.get(55), 1);
  assert.doesNotThrow(() => audit('const v = <plot kind="line" values={[1,2]} colors={["#123456"]} smooth area />;', 'plot.ui.tsx'));
  reset();
  assert.throws(() => describe(() => h('column', {}, Array.from({ length: 8 }, (_, i) =>
    plot({ key: String(i), kind: 'line', smooth: true, area: true, values: Array(128).fill(1) }))), view), /plot geometry budget/);
});

test('many individually valid plots share one geometry budget', () => {
  reset();
  assert.throws(() => describe(() => h('column', {}, Array.from({ length: 64 }, (_, i) => plot({ key: String(i), kind: 'pie' }))), view), /plot geometry budget/);
});

test('explicit plot domains encode exact bounds and reject misleading scales', () => {
  reset();
  const wire = describe(() => plot({domainMin: 0, domainMax: 30}), view);
  const fields = new Map(Array.from({length: wire[5]}, (_, i) => wire.slice(6+i*2,8+i*2)));
  assert.equal(fields.get(85), 0); assert.equal(fields.get(86), 30);
  for (const props of [
    {domainMin: 0}, {domainMax: 30}, {domainMin: 0, domainMax: 10},
    {domainMin: 10, domainMax: 30}, {domainMin: 20, domainMax: 20},
    {domainMin: -Infinity, domainMax: 30}, {domainMin: 0, domainMax: NaN},
    {domainMin: '0', domainMax: 30}, {domainMin: 0, domainMax: 1e10},
    {kind: 'pie', domainMin: 0, domainMax: 30},
  ]) { reset(); assert.throws(() => describe(() => plot(props), view), /UI_PLOT/); }
});

test('rounded bar paths share the frame geometry budget', () => {
  reset();
  assert.throws(() => describe(() => h('column', {}, Array.from({length:17}, (_, i) =>
    plot({key:String(i),values:Array(128).fill(1)}))), view), /plot geometry budget/);
});


test('pie layout encodes normalized geometry and rejects unsupported layouts', () => {
  const pieLayout = { center: [0.5, 0.44], innerRadius: 0.4, outerRadius: 0.68 };
  reset();
  const wire = describe(() => plot({ kind: 'pie', pieLayout }), view);
  assert.ok(wire.includes(JSON.stringify(pieLayout)));
  for (const emphasisScale of [0, 5, 64]) {
    reset();
    const layout = {...pieLayout, emphasisScale};
    assert.ok(describe(() => plot({kind:'pie',pieLayout:layout}),view).includes(JSON.stringify(layout)));
  }
  for (const layout of [null, {}, {...pieLayout, center: [0.5]}, {...pieLayout, center: [0, Infinity]},
    {...pieLayout, innerRadius: -1}, {...pieLayout, outerRadius: 1.1}, {...pieLayout, innerRadius: 0.68},
    {...pieLayout, outerRadius: '0.7'}, {...pieLayout, unknown: 1},
    ...[-1,65,Infinity,NaN,'5',null].map(emphasisScale => ({...pieLayout,emphasisScale}))]) {
    reset(); assert.throws(() => describe(() => plot({kind:'pie',pieLayout:layout}),view), /UI_PLOT/);
  }
  reset(); assert.throws(() => describe(() => plot({pieLayout}),view), /UI_PLOT/);
});
