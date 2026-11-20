import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import { compileUtilities } from '../compiler/tailwind.mjs';
import { configureClasses } from '../src/style/classes.js';
import { h, describe, useLocal, reset } from '../src/internal.js';
import { rendered, byTestId } from './support/wire.mjs';

// Liuli's Switch recipe (its own contact shadow swapped for a stock one), plain and loading.
const recipe = 'min-h-8 track:rounded-full track:bg-line-strong/45 checked:track:bg-accent-solid thumb:rounded-full thumb:bg-knob thumb:border thumb:border-text/12 thumb:shadow-sm disabled:track:bg-text/12 disabled:checked:track:bg-text/20';
const loading = `${recipe} disabled:track:bg-line-strong/45 disabled:checked:track:bg-accent-solid`;
const agree = 'flex-row items-center gap-3 rounded-xl px-4 py-3 bg-surface-sunken has-checked:bg-accent-solid has-checked:text-on-accent-solid';
before(async () => configureClasses(await compileUtilities(new Set([recipe, loading, agree].flatMap(names => names.split(' '))))));
after(() => configureClasses([]));

const toggle = (className, value, disabled) => byTestId(rendered(() => h('page', {},
  h('switch', { 'data-testid': 's', 'aria-label': 'S', value, disabled, onChange() {}, className }))), 's').props;
// What the core paints: a control that reads checked takes its checked style over the base, field by field.
const painted = ({ checkedStyle, onCheckedChange, ...props }) => props.checked === 1 && checkedStyle ? { ...props, ...checkedStyle } : props;
const track = color => JSON.stringify([10, 16384, 90, 16384, 91, 16384, 92, 16384, 93, 16384, 6, color]);

test('a bound switch paints exactly as it did when checked: resolved from its value', () => {
  // The track colour each recipe drew before, by [checked, disabled].
  const before = {
    recipe: { 'false,false': 'line-strong/45', 'false,true': 'text/12', 'true,false': 'accent-solid', 'true,true': 'text/20' },
    loading: { 'false,false': 'line-strong/45', 'false,true': 'line-strong/45', 'true,false': 'accent-solid', 'true,true': 'accent-solid' },
  };
  for (const [name, className] of [['recipe', recipe], ['loading', loading]]) {
    const thumb = toggle(className, false, false).thumbStyle;
    for (const value of [false, true]) for (const disabled of [false, true]) {
      const props = toggle(className, value, disabled), shown = painted(props);
      assert.equal(shown.trackStyle, track(before[name][`${value},${disabled}`]), `${name} ${value} ${disabled}`);
      assert.equal(shown.thumbStyle, thumb, 'the thumb never moves with checked');
      // The checked style carries only the part checked: changes, whole.
      assert.deepEqual(Object.keys(props.checkedStyle), ['trackStyle']);
      assert.deepEqual(toggle(className, !value, disabled).checkedStyle, props.checkedStyle, 'independent of the value');
    }
  }
});

test('a has-checked: container paints as it did, its style laid on while a control inside reads checked', () => {
  const row = value => byTestId(rendered(() => h('page', {}, h('view', { 'data-testid': 'row', className: agree },
    h('checkbox', { 'aria-label': 'Agree', value, onChange() {} })))), 'row').props;
  const off = row(false), on = row(true);
  assert.deepEqual(off, on, 'the record no longer depends on the descendants');
  const { hasCheckedStyle, ...base } = on;
  assert.deepEqual([base.background, base.color], ['surface-sunken', undefined]);
  assert.deepEqual(hasCheckedStyle, { background: 'accent-solid', color: 'on-accent-solid' });
});

test('defaultChecked and defaultValue start an unbound control once and are refused on a bound one', () => {
  reset();
  const options = [{ value: 'm', label: 'Monthly' }, { value: 'y', label: 'Yearly' }];
  for (const tag of ['checkbox', 'switch']) {
    const props = rendered(() => h(tag, { defaultChecked: true, 'aria-label': 'x' })).props;
    // The value buffer carries no booleans: the flag travels as 0/1.
    assert.deepEqual([props.checked, props.defaultValue], [undefined, 1]);
    assert.equal(rendered(() => h(tag, { defaultChecked: false, 'aria-label': 'x' })).props.defaultValue, 0);
    assert.throws(() => describe(() => h(tag, { defaultChecked: true, value: false, onChange() {}, 'aria-label': 'x' }), {}), /UI_CHECK: defaultChecked starts a control without value or bind/);
    assert.throws(() => describe(() => h(tag, { defaultChecked: true, bind: useLocal('on', false), 'aria-label': 'x' }), {}), /UI_CHECK: defaultChecked/);
    assert.throws(() => describe(() => h(tag, { defaultChecked: 1, 'aria-label': 'x' }), {}), /UI_CHECK: defaultChecked must be boolean/);
  }
  // A slider's start snaps as its value would.
  const slider = rendered(() => h('slider', { defaultValue: 47, step: 10, 'aria-label': 'Volume' })).props;
  assert.deepEqual([slider.sliderValue, slider.defaultValue], [undefined, 50]);
  assert.throws(() => describe(() => h('slider', { defaultValue: 5, value: 5, onChange() {}, 'aria-label': 'x' }), {}), /UI_SLIDER: defaultValue starts a slider without value or bind/);
  assert.throws(() => describe(() => h('slider', { defaultValue: '5', 'aria-label': 'x' }), {}), /UI_SLIDER: defaultValue must be a finite number/);
  assert.throws(() => describe(() => h('slider', { defaultValue: 5, values: [1, 9], thumbLabels: ['a', 'b'], 'aria-label': 'x' }), {}), /UI_SLIDER: defaultValue belongs to a slider with one value/);
  // A select shows its start's label until the core shows a later choice.
  const select = rendered(() => h('select', { options, defaultValue: 'y', placeholder: 'Plan', 'aria-label': 'Plan' }));
  assert.deepEqual([select.props.inputValue, select.props.defaultValue, select.text], [undefined, 'y', 'Yearly']);
  for (const props of [{ defaultValue: 'q' }, { defaultValue: 'y', value: 'm', onChange() {} }])
    assert.throws(() => describe(() => h('select', { options, 'aria-label': 'Plan', ...props }), {}), /UI_SELECT: defaultValue names an option of a select without value or bind/);
});
