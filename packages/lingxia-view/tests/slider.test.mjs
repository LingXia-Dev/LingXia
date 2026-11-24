import { test } from 'node:test';
import assert from 'node:assert/strict';
import { h, describe, prepare, discard, dispatch, useLocal, reset } from '../src/internal.js';
import { tree, rendered, byTestId, points } from './support/wire.mjs';
import { releasePress } from '../src/runtime/press.js';
import { compileUtilities } from '../compiler/tailwind.mjs';
import { configureClasses } from '../src/style/classes.js';

test('slider clamps and snaps numeric proposals including a partial final interval', () => {
  reset(); let calls = 0;
  const render = () => {
    const value = useLocal('value', 0);
    return h('slider', { 'aria-label': 'Volume', min: 0, max: 10, step: 3, value: value.value, onChange: next => { calls++; value.set(next); } });
  };
  let slider = rendered(render);
  assert.equal(slider.props.sliderValue, 0);
  for (const [proposal, expected] of [[4, 3], [8, 9], [9.6, 10], [20, 10], [-1, 0]]) {
    const handle = slider.props.onSliderChange;
    assert.equal(dispatch(handle, proposal), true);
    slider = rendered(render);
    assert.equal(slider.props.sliderValue, expected);
    assert.equal(dispatch(handle, 1), false, 'a superseded handle is dead');
  }
  assert.equal(calls, 4, 'a proposal equal to the value does not call onChange');
  const handle = slider.props.onSliderChange;
  for (const value of [undefined, '3', true, NaN, Infinity, 1e10]) assert.equal(dispatch(handle, value), false);
  const pending = tree(prepare(render, {})).props.onSliderChange; discard();
  assert.equal(dispatch(pending, 3), false); assert.equal(dispatch(handle, 3), true);
});

test('slider validates bounds, finite numeric binding, labels and disabled callbacks', () => {
  reset();
  const render = () => h('slider', { 'aria-label': 'Intensity', bind: useLocal('intensity', 0.5), min: -1, max: 1, step: 0.25 });
  assert.equal(dispatch(rendered(render).props.onSliderChange, -0.6), true);
  assert.equal(rendered(render).props.sliderValue, -0.5);
  for (const props of [{ value: null }, { value: '1' }, { value: NaN }, { min: null }, { max: 0 }, { step: 0 }, { step: 101 }, { step: 0.00000001 },
    { min: 1e9, max: 1e9 + 1 }, { 'aria-label': '' }, { onChange: 1 }, { bind: { value: 1, set() {} } }])
    assert.throws(() => describe(() => h('slider', { 'aria-label': 'Range', ...props }), {}), /UI_SLIDER/);
  assert.throws(() => describe(() => h('slider', { 'aria-label': 'Range', min: 10, max: 5, step: 1 }), {}), /UI_SLIDER: range takes finite min < max within ±1e9 and step > 0 .*?; got min=10, max=5, step=1/);
  assert.throws(() => describe(() => h('slider', { 'aria-label': 'Range', bind: useLocal('bad', '1') }), {}), /UI_SLIDER/);
  assert.throws(() => describe(() => h('slider', { 'aria-label': 'Range' }, 'label'), {}), /UI_CHILD/);
  const disabled = rendered(() => h('slider', { 'aria-label': 'Range', disabled: true, onChange() {} }));
  assert.equal(disabled.props.onSliderChange, undefined);
  assert.equal(disabled.props.enabled, 0);
});

test('an unbound slider leaves its value to the core and reports each value the core sends', () => {
  reset(); const calls = [];
  const render = () => h('slider', { 'aria-label': 'Volume', min: 0, max: 10, step: 2,
    onChange: next => calls.push(['change', next]), onChangeEnd: next => calls.push(['end', next]) });
  const slider = rendered(render);
  // No `sliderValue` on the wire marks it unbound; the core starts it at min.
  assert.equal(slider.props.sliderValue, undefined);
  assert.equal(slider.props.sliderMin, 0);
  dispatch(rendered(render).props.onSliderPress);
  assert.equal(dispatch(rendered(render).props.onSliderChange, 4), true);
  assert.equal(dispatch(rendered(render).props.onSliderChange, 6), true);
  assert.equal(releasePress(), true);
  assert.deepEqual(calls, [['change', 4], ['change', 6], ['end', 6]]);
  // Without a callback it still takes a drag, so it keeps a handler and is not drawn disabled.
  const silent = rendered(() => h('slider', { 'aria-label': 'Silent' }));
  assert.equal(silent.props.sliderValue, undefined); assert.equal(silent.props.enabled, undefined);
  assert.equal(dispatch(silent.props.onSliderChange, 30), true);
  // A controlled slider is unchanged: its value wired, no handler without a callback.
  const fixed = rendered(() => h('slider', { 'aria-label': 'Fixed', value: 3 }));
  assert.equal(fixed.props.sliderValue, 3); assert.equal(fixed.props.onSliderChange, undefined);
  assert.equal(rendered(() => h('slider', { 'aria-label': 'Off', disabled: true })).props.onSliderChange, undefined);
});

test('onChangeEnd hears a change when its press lifts, and at once without a press', () => {
  reset(); const calls = [];
  const render = () => {
    const value = useLocal('value', 0);
    return h('slider', { 'aria-label': 'Volume', value: value.value, onChange: next => { calls.push(['change', next]); value.set(next); }, onChangeEnd: next => calls.push(['end', next]) });
  };
  const step = value => { dispatch(rendered(render).props.onSliderChange, value); };
  step(10);
  assert.deepEqual(calls, [['change', 10], ['end', 10]], 'a key or assistive step ends at once');
  calls.length = 0;
  dispatch(rendered(render).props.onSliderPress);
  step(20); step(30);
  assert.deepEqual(calls, [['change', 20], ['change', 30]], 'a held press keeps it open');
  assert.equal(releasePress(), true);
  assert.deepEqual(calls.slice(2), [['end', 30]], 'the release ends it once, with the last value');
  calls.length = 0;
  dispatch(rendered(render).props.onSliderPress);
  releasePress();
  assert.deepEqual(calls, [], 'a press that changed nothing ends nothing');
  assert.throws(() => describe(() => h('slider', { 'aria-label': 'x', onChangeEnd: 1 }), {}), /UI_SLIDER: onChangeEnd/);
});

test('two values make a named group of two sliders over one track that cannot cross', async () => {
  reset(); const calls = [];
  configureClasses(await compileUtilities(new Set(['w-64', 'mx-4', 'h-10', 'px-2', 'text-accent', 'thumb:bg-accent', 'bg-surface'])));
  const render = () => {
    const range = useLocal('range', [20, 60]);
    return h('page', {}, h('slider', { 'aria-label': 'Price', thumbLabels: ['From', 'To'], values: range.value, step: 10, 'data-testid': 'price',
      className: 'w-64 mx-4 h-10 px-2 text-accent thumb:bg-accent bg-surface',
      onChange: next => { calls.push(['change', next]); range.set(next); }, onChangeEnd: next => calls.push(['end', next]) }));
  };
  const parts = () => { const root = rendered(render); return [byTestId(root, 'price'), byTestId(root, 'price-upper'), byTestId(root, 'price-lower')]; };
  let [group, upper, lower] = parts();
  assert.deepEqual([group.props.role, group.props.accessibilityLabel], ['group', 'Price']);
  assert.deepEqual(group.children.map(child => child.testId), ['price-lower', 'price-upper'], 'in reading order; the upper thumb paints above');
  assert.deepEqual([upper.props.accessibilityLabel, upper.props.sliderValue, upper.props.sliderOther, upper.props.sliderThumb], ['To', 60, 20, 'upper']);
  assert.deepEqual([lower.props.accessibilityLabel, lower.props.sliderValue, lower.props.sliderOther, lower.props.sliderThumb], ['From', 20, 60, 'lower']);
  // Placement and the box go to the group; what the control draws with stays on each slider.
  assert.deepEqual(points([group.props.width, group.props.height, lower.props.width, lower.props.height, upper.props.width]), [256, 40, undefined, 40, undefined]);
  assert.ok(group.props.background !== undefined && upper.props.background === undefined && lower.props.background === undefined);
  assert.ok(group.props.thumbStyle === undefined && upper.props.thumbStyle !== undefined && lower.props.thumbStyle !== undefined);
  assert.deepEqual([upper.props.color, lower.props.color, upper.props.position], ['accent', 'accent', 'absolute']);
  dispatch(lower.props.onSliderChange, 40);
  assert.deepEqual(calls, [['change', [40, 60]], ['end', [40, 60]]]);
  [group, upper, lower] = parts();
  dispatch(lower.props.onSliderChange, 90);
  assert.deepEqual(calls[2], ['change', [60, 60]], 'the lower thumb stops at the upper');
  [group, upper, lower] = parts();
  dispatch(upper.props.onSliderChange, 10);
  assert.equal(calls.length, 4, 'and the upper, already there, does not move below it');
  calls.length = 0;
  dispatch(upper.props.onSliderPress);
  dispatch(upper.props.onSliderChange, 80);
  [group, upper, lower] = parts();
  dispatch(upper.props.onSliderChange, 100);
  releasePress();
  assert.deepEqual(calls, [['change', [60, 80]], ['change', [60, 100]], ['end', [60, 100]]]);
  configureClasses([]);
  const bad = props => () => describe(() => h('slider', { 'aria-label': 'Price', thumbLabels: ['From', 'To'], values: [1, 2], ...props }), {});
  assert.throws(bad({ values: [5, 1] }), /UI_SLIDER: values/);
  assert.throws(bad({ values: [1] }), /UI_SLIDER: values/);
  assert.throws(bad({ thumbLabels: ['From'] }), /UI_A11Y/);
  assert.throws(bad({ thumbLabels: undefined }), /UI_A11Y/);
  assert.throws(bad({ value: 3 }), /UI_SLIDER: value belongs/);
  assert.throws(bad({ 'aria-label': undefined }), /UI_A11Y/);
});
