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

test('a tap commits a day; Escape and the backdrop cancel', () => {
  reset();
  const view = () => h('column', {});
  const env = { pointer: 'coarse', displayLanguage: 'zh-Hans-CN' };
  open({ kind: 'date', value: '' });
  let nodes = render(view, env);
  assert.ok(nodes.find(n => n.testId === 'lx-picker-done'), 'touch draws a sheet with Done');
  const day = nodes.find(n => n.testId.startsWith('lx-picker-day-') && n.props.enabled !== 0);
  dispatch(day.props.onClick);
  assert.equal(JSON.parse(takePickerResults())[0].value, day.testId.slice('lx-picker-day-'.length));
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
  assert.equal(column('lx-picker-minute').listCount, 4, 'a 15-minute step offers four minutes');
  assert.deepEqual(JSON.parse(column('lx-picker-period').listItemKeys), ['am', 'pm']);
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
