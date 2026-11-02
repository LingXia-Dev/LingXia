import { test } from 'node:test';
import assert from 'node:assert/strict';
import { h, prepare, commit, dispatch, useLocal, reset } from '../src/internal.js';
import { openPicker, pickerKey, takePickerResults, hasPicker } from '../src/runtime/picker.js';
import { decode } from './support/wire.mjs';

const open = request => openPicker({ identity: '/i:0:0/i:0:0', ...request });
const render = (view, env) => { const out = decode(prepare(view, { env })); commit(); return out; };

test('a date picker opens on its editor, moves by keys and commits an ISO date', () => {
  reset();
  const view = () => h('column', {});
  const env = { pointer: 'fine', firstWeekday: 2, displayLanguage: 'en-GB' };
  assert.equal(open({ kind: 'text' }), false);
  assert.equal(open({ kind: 'date', value: '2026-10-29', min: '2026-10-05', max: '2026-11-20' }), true);
  let nodes = render(view, env);
  assert.equal(nodes.find(n => n.testId === 'lx-picker-title').text, 'October 2026');
  const cells = nodes.filter(n => n.testId.startsWith('lx-picker-day-'));
  assert.equal(cells.length, 42);
  assert.equal(cells[0].testId, 'lx-picker-day-2026-09-28', 'weeks start on Monday');
  assert.equal(nodes.find(n => n.testId === 'lx-picker-day-2026-10-04').props.enabled, 0, 'before min is inert');
  for (const key of ['PageDown', 'ArrowDown']) assert.equal(pickerKey(key), true);
  nodes = render(view, env);
  assert.equal(nodes.find(n => n.testId === 'lx-picker-title').text, 'November 2026');
  assert.equal(pickerKey('ArrowDown'), true, 'clamped to max');
  assert.equal(pickerKey('Enter'), true);
  assert.deepEqual(JSON.parse(takePickerResults()), [{ identity: '/i:0:0/i:0:0', value: '2026-11-20' }]);
  assert.equal(hasPicker(), false);
});

test('on touch a tap picks a day and Done commits it; Cancel and Escape keep the old value', () => {
  reset();
  const view = () => h('column', {});
  const env = { pointer: 'coarse', displayLanguage: 'zh-Hans-CN' };
  open({ kind: 'date', value: '2026-10-29' });
  let nodes = render(view, env);
  dispatch(nodes.find(n => n.testId === 'lx-picker-day-2026-10-21').props.onClick);
  assert.equal(takePickerResults(), '[]', 'a tap alone commits nothing');
  nodes = render(view, env);
  assert.equal(nodes.find(n => n.testId === 'lx-picker-day-2026-10-21').props.stateSelected, 1, 'the picked day shows as chosen');
  dispatch(nodes.find(n => n.testId === 'lx-picker-cancel').props.onClick);
  assert.deepEqual(JSON.parse(takePickerResults())[0].value, null);
  open({ kind: 'date', value: '2026-10-29' });
  nodes = render(view, env);
  dispatch(nodes.find(n => n.testId === 'lx-picker-day-2026-10-21').props.onClick);
  nodes = render(view, env);
  dispatch(nodes.find(n => n.testId === 'lx-picker-done').props.onClick);
  assert.equal(JSON.parse(takePickerResults())[0].value, '2026-10-21');
  open({ kind: 'date', value: '' }); pickerKey('Escape');
  assert.deepEqual(JSON.parse(takePickerResults())[0].value, null);
});

test('a time picker honours the step and the hour cycle, and takes typed digits', () => {
  reset();
  const view = () => h('column', {});
  const env = { pointer: 'fine', hourCycle: 'h12', displayLanguage: 'en-US' };
  open({ kind: 'time', value: '13:15', step: 900 });
  const nodes = render(view, env);
  const column = testId => nodes.find(n => n.testId === testId).children[0].props;
  const minutes = JSON.parse(column('lx-picker-minute').listItemKeys);
  assert.deepEqual(minutes.slice(4, 8), ['1:0', '1:15', '1:30', '1:45'], 'a 15-minute step offers four minutes, repeated around the middle');
  assert.equal(minutes.length, 12);
  assert.deepEqual(JSON.parse(column('lx-picker-period').listItemKeys), ['0:am', '0:pm'], 'AM/PM does not repeat');
  pickerKey('ArrowDown'); pickerKey('Enter');
  assert.equal(JSON.parse(takePickerResults())[0].value, '13:30');
  open({ kind: 'time', value: '13:15', step: 900 });
  for (const key of ['0', '7', '4', '5']) pickerKey(key);
  pickerKey('Enter');
  assert.equal(JSON.parse(takePickerResults())[0].value, '07:45');
});

test('a select picker lists options, skips disabled ones and types ahead', () => {
  reset();
  const options = [{ value: 'a', label: 'Apple' }, { value: 'b', label: 'Banana', enabled: false }, { value: 'c', label: 'Cherry' }, { value: 'd', label: 'Date' }];
  assert.equal(open({ kind: 'select', options, selected: 0, label: 'Fruit' }), true);
  const nodes = render(() => h('column', {}), { pointer: 'fine' });
  assert.equal(nodes.find(n => n.testId === 'lx-picker-option-1').props.enabled, 0);
  pickerKey('ArrowDown');
  pickerKey('Enter');
  assert.deepEqual(JSON.parse(takePickerResults()), [{ identity: '/i:0:0/i:0:0', value: 'c', index: 2 }], 'Banana is skipped');
  open({ kind: 'select', options, selected: 0 });
  pickerKey('d'); pickerKey('Enter');
  assert.equal(JSON.parse(takePickerResults())[0].value, 'd');
});

test('on touch every picker sheet opens with Cancel, a title and Done on top', () => {
  for (const [request, title] of [
    [{ kind: 'date', value: '2026-10-29' }, 'October 2026'],
    [{ kind: 'time', value: '18:05' }, '6:05 PM'],
    [{ kind: 'select', options: [{ value: 'a', label: 'A' }, { value: 'b', label: 'B' }], selected: 0, label: 'Plan' }, 'Plan'],
  ]) {
    reset();
    open(request);
    const nodes = render(() => h('column', {}), { pointer: 'coarse', hourCycle: 'h12', displayLanguage: 'en-US' });
    const sheet = nodes.find(n => n.testId === 'lx-picker');
    const toolbar = sheet.children[0];
    assert.deepEqual(toolbar.children.map(n => n.testId), ['lx-picker-cancel', 'lx-picker-title', 'lx-picker-done'], request.kind);
    assert.equal(toolbar.children[1].text, title);
    dispatch(toolbar.children[2].props.onClick);
    assert.equal(hasPicker(), false, `${request.kind}: Done commits`);
  }
});
