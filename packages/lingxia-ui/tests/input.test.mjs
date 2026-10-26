import { test } from 'node:test';
import assert from 'node:assert/strict';
import { h, describe, prepare, discard, dispatch, useLocal, reset } from '../src/internal.js';
import { decode, rendered } from './support/wire.mjs';

test('editor labels are distinct from values and placeholder hints', () => {
  for (const tag of ['input', 'textarea', 'search']) {
    const { props } = rendered(() => h(tag, { value: 'Ada', placeholder: 'Your name', accessibilityLabel: 'Name' }));
    assert.deepEqual([props.accessibilityLabel, props.inputValue, props.placeholder], ['Name', 'Ada', 'Your name']);
    assert.throws(() => describe(() => h(tag, { accessibilityLabel: '  ' }), {}), /UI_INPUT/);
  }
});

test('local string bindings receive only typed committed text events', () => {
  reset();
  const render = () => { const draft = useLocal('draft', ''); return h('column', {}, h('input', { bind: draft }), h('text', {}, draft.value)); };
  const handle = rendered(render).children[0].props.onInput;
  assert.equal(dispatch(handle), false);
  assert.equal(dispatch(handle, 42), false);
  assert.equal(dispatch(handle, '你好😀\n"quoted"'), true);
  const [input, text] = rendered(render).children;
  assert.equal(input.props.inputValue, '你好😀\n"quoted"');
  assert.equal(text.text, '你好😀\n"quoted"');
  assert.equal(dispatch(handle, 'stale'), false);
});

test('input callbacks survive discarded candidates and disabled inputs register none', () => {
  reset(); let received = '';
  const render = () => h('textarea', { value: 'a', onSubmit: value => received = value });
  const old = rendered(render).props.onSubmit;
  const pending = decode(prepare(render, {}))[0].props.onSubmit; discard();
  assert.equal(dispatch(pending, 'bad'), false); assert.equal(dispatch(old, 'ok'), true); assert.equal(received, 'ok');
  const disabled = rendered(() => h('input', { disabled: true, onInput() {}, onSubmit() {} }));
  assert.equal(disabled.props.onInput, undefined); assert.equal(disabled.props.onSubmit, undefined);
  assert.equal(disabled.props.enabled, 0);
});

test('input descriptions validate text, bindings, children and props', () => {
  for (const props of [{ value: null }, { value: 1 }, { value: '中'.repeat(22000) }, { value: '\uD800' }, { placeholder: false }, { secure: 1 },
    { autoFocus: 1 }, { keyboard: 'url' }, { keyboard: 'search' }, { onInput: 1 }, { bind: { value: 'x', set() {} } }])
    assert.throws(() => describe(() => h('input', props), {}), /UI_INPUT/);
  assert.throws(() => describe(() => h('textarea', {}, 'child'), {}), /UI_CHILD/);
  assert.throws(() => describe(() => h('input', { onClick() {} }), {}), /UI_PROP/);
  assert.throws(() => describe(() => h('input', { bind: useLocal('bad', 42) }), {}), /UI_INPUT/);
  assert.throws(() => describe(() => h('input', { bind: useLocal('ambiguous', ''), value: 'x' }), {}), /UI_INPUT/);
});

test('text event validation rejects oversized and malformed UTF-16 payloads', () => {
  reset(); let count = 0;
  const handle = rendered(() => h('input', { onInput() { count++; } })).props.onInput;
  assert.equal(dispatch(handle, '中'.repeat(22000)), false);
  assert.equal(dispatch(handle, '\uDFFF'), false);
  assert.equal(count, 0);
  assert.equal(dispatch(handle, 'valid'), true); assert.equal(count, 1);
});

test('bind rejects cells removed from the current candidate', () => {
  reset(); let saved;
  describe(() => { saved = useLocal('draft', 'old'); return h('text', {}, saved.value); }, {});
  describe(() => h('text', {}, 'removed'), {});
  assert.throws(() => describe(() => h('input', { bind: saved }), {}), /UI_INPUT/);
});

test('ASCII fast validation preserves the exact UTF-8 byte limit', () => {
  reset(); let received = '';
  const handle = rendered(() => h('input', { onInput: value => received = value })).props.onInput;
  assert.equal(dispatch(handle, 'x'.repeat(65536)), true); assert.equal(received.length, 65536);
  assert.equal(dispatch(handle, 'x'.repeat(65537)), false); assert.equal(received.length, 65536);
});

test('search binds committed text and submits through a search keyboard', () => {
  reset(); let submitted = '';
  const render = () => h('search', { bind: useLocal('query', ''), onSubmit: value => submitted = value });
  const first = rendered(render);
  assert.equal(first.tag, 'search'); assert.equal(first.props.keyboard, 'search');
  assert.equal(dispatch(first.props.onInput, 'coffee'), true);
  const next = rendered(render); assert.equal(next.props.inputValue, 'coffee');
  assert.equal(dispatch(next.props.onSubmit, 'coffee'), true); assert.equal(submitted, 'coffee');
  for (const props of [{ secure: true }, { keyboard: 'text' }]) assert.throws(() => describe(() => h('search', props), {}), /UI_PROP/);
  assert.throws(() => describe(() => h('search', {}, 'child'), {}), /UI_CHILD/);
  const disabled = rendered(() => h('search', { disabled: true, onInput() {}, onSubmit() {} }));
  assert.equal(disabled.props.onInput, undefined); assert.equal(disabled.props.onSubmit, undefined);
});

test('auto height is explicit, while unsupported auto dimensions are rejected', () => {
  reset();
  assert.equal(rendered(() => h('textarea', { style: { height: 'auto' } })).props.heightAuto, 1);
  assert.throws(() => describe(() => h('textarea', { style: { maxHeight: 'auto' } }), {}), /UI_STYLE_VALUE/);
});

test('submitOnEnter is an explicit textarea flag with a normal submit callback', () => {
  reset(); let sent;
  const textarea = rendered(() => h('textarea', { submitOnEnter: true, onSubmit: v => sent = v }));
  assert.equal(textarea.props.submitOnEnter, 1);
  assert.equal(dispatch(textarea.props.onSubmit, 'message'), true);
  assert.equal(sent, 'message');
  for (const tag of ['input', 'search']) assert.throws(() => describe(() => h(tag, { submitOnEnter: true }), {}), /UI_PROP/);
  for (const value of [1, 'true', null]) assert.throws(() => describe(() => h('textarea', { submitOnEnter: value }), {}), /UI_INPUT/);
});

test('focus styles lower to paint deltas on editors and are rejected elsewhere', async () => {
  const { configureClasses } = await import('../src/internal.js');
  configureClasses([
    { name: 'focus:bg-surface', when: { state: 'focus' }, style: { backgroundColor: 'surface' } },
    { name: 'focus:p-4', when: { state: 'focus' }, style: { paddingTop: 16 } },
  ]);
  assert.deepEqual(rendered(() => h('input', { className: 'focus:bg-surface' })).props.focusStyle, { background: 'surface' });
  assert.throws(() => describe(() => h('column', { className: 'focus:bg-surface' }), {}), /UI_STYLE: focus: requires an editor/);
  assert.throws(() => describe(() => h('input', { className: 'focus:p-4' }), {}), /UI_STYLE: focus: supports paint properties only/);
  configureClasses([]);
});

test('editor attributes are validated, normalized and wired for the platform input connection', () => {
  const props = rendered(() => h('input', { value: '', maxLength: 6, autoComplete: 'one-time-code', enterKeyHint: 'go', autoCapitalize: 'off', spellCheck: false })).props;
  assert.deepEqual([props.maxLength, props.autoComplete, props.enterKeyHint, props.autoCapitalize, props.spellCheck], [6, 'one-time-code', 'go', 'none', 0]);
  const number = rendered(() => h('input', { value: '', keyboard: 'number', min: 0, max: 9.5, step: 0.5 })).props;
  assert.deepEqual([number.inputMin, number.inputMax, number.inputStep], [0, 9.5, 0.5]);
  const date = rendered(() => h('input', { value: '', keyboard: 'date', min: '2024-01-01' })).props;
  assert.deepEqual([date.keyboard, date.inputMin], ['date', '2024-01-01']);
  for (const [tag, props, message] of [
    ['input', { maxLength: 0 }, /maxLength/], ['input', { autoComplete: 'password' }, /autoComplete must be one of/],
    ['textarea', { keyboard: 'time' }, /need <input>/], ['input', { min: 1 }, /need keyboard number, date or time/],
    ['input', { keyboard: 'number', min: 5, max: 1 }, /min exceeds max/], ['input', { keyboard: 'date', min: '01/02/2024' }, /ISO form/],
    ['input', { keyboard: 'date', step: 1 }, /step is a positive number/], ['input', { keyboard: 'search' }, /keyboard/],
    ['search', { spellCheck: 'no' }, /spellCheck/]])
    assert.throws(() => describe(() => h(tag, { value: '', ...props }), {}), message);
});
