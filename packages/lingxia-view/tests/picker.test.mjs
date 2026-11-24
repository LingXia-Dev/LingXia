import { test } from 'node:test';
import assert from 'node:assert/strict';
import { h, prepare, commit, dispatch, useLocal, reset } from '../src/internal.js';
import { openPicker, pickerKey, takePickerResults, hasPicker } from '../src/runtime/picker.js';
import { decode } from './support/wire.mjs';
import { publishHost } from '../src/host.js';

const open = request => openPicker({ identity: '/i:0:0/i:0:0', ...request });
const render = (view, env) => { const out = decode(prepare(view, { env })); commit(); return out; };

test('a date picker opens on its editor, moves by keys and commits an ISO date', () => {
  reset();
  const view = () => h('view', {});
  const env = { pointer: 'fine', firstWeekday: 1, language: 'en-GB' };
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
  const view = () => h('view', {});
  const env = { pointer: 'coarse', language: 'zh-Hans-CN' };
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

test("the sheet speaks the page's language", () => {
  reset();
  open({ kind: 'date', value: '2026-10-29' });
  const nodes = render(() => h('view', {}), { pointer: 'coarse', language: 'en' });
  assert.equal(nodes.find(n => n.testId === 'lx-picker-title').text, 'October 2026');
  pickerKey('Escape'); takePickerResults();
});

test('a time picker honours the step and the hour cycle, and takes typed digits', () => {
  reset();
  const view = () => h('view', {});
  const env = { pointer: 'fine', hourCycle: 'h12', language: 'en-US' };
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
  const nodes = render(() => h('view', {}), { pointer: 'fine' });
  assert.equal(nodes.find(n => n.testId === 'lx-picker-option-1').props.enabled, 0);
  pickerKey('ArrowDown');
  pickerKey('Enter');
  assert.deepEqual(JSON.parse(takePickerResults()), [{ identity: '/i:0:0/i:0:0', value: 'c', index: 2 }], 'Banana is skipped');
  open({ kind: 'select', options, selected: 0 });
  pickerKey('d'); pickerKey('Enter');
  assert.equal(JSON.parse(takePickerResults())[0].value, 'd');
});

test('a select sheet without a value ticks nothing until a tap picks, and Done then commits the tap', () => {
  reset();
  const options = [{ value: 'de', label: 'Germany' }, { value: 'fr', label: 'France' }];
  open({ kind: 'select', options, selected: null, label: 'Country' });
  const ticked = () => render(() => h('view', {}), { pointer: 'coarse' })
    .filter(n => /^lx-picker-option-/.test(n.testId) && n.props.stateSelected === 1).map(n => n.testId);
  assert.deepEqual(ticked(), [], 'focus is not selection');
  let nodes = render(() => h('view', {}), { pointer: 'coarse' });
  dispatch(nodes.find(n => n.testId === 'lx-picker-done').props.onClick);
  assert.deepEqual(JSON.parse(takePickerResults()), [{ identity: '/i:0:0/i:0:0', value: null }], 'Done with no pick changes nothing');
  open({ kind: 'select', options, selected: null, label: 'Country' });
  nodes = render(() => h('view', {}), { pointer: 'coarse' });
  dispatch(nodes.find(n => n.testId === 'lx-picker-option-1').props.onClick);
  assert.deepEqual(ticked(), ['lx-picker-option-1']);
  nodes = render(() => h('view', {}), { pointer: 'coarse' });
  dispatch(nodes.find(n => n.testId === 'lx-picker-done').props.onClick);
  assert.equal(JSON.parse(takePickerResults())[0].value, 'fr');
});

test('on touch every picker sheet opens with Cancel, a title and Done on top', () => {
  for (const [request, title] of [
    [{ kind: 'date', value: '2026-10-29' }, 'October 2026'],
    [{ kind: 'time', value: '18:05' }, '6:05 PM'],
    [{ kind: 'select', options: [{ value: 'a', label: 'A' }, { value: 'b', label: 'B' }], selected: 0, label: 'Plan' }, 'Plan'],
  ]) {
    reset();
    open(request);
    const nodes = render(() => h('view', {}), { pointer: 'coarse', hourCycle: 'h12', language: 'en-US' });
    const sheet = nodes.find(n => n.testId === 'lx-picker');
    const toolbar = sheet.children[0];
    assert.deepEqual(toolbar.children.map(n => n.testId), ['lx-picker-cancel', 'lx-picker-title', 'lx-picker-done'], request.kind);
    assert.equal(toolbar.children[1].text, title);
    dispatch(toolbar.children[2].props.onClick);
    assert.equal(hasPicker(), false, `${request.kind}: Done commits`);
  }
});

test('on touch an empty date field opens with nothing chosen; today is a ring, and Done waits for a tap', () => {
  reset();
  const view = () => h('view', {});
  const env = { pointer: 'coarse', language: 'en' };
  const day = id => nodes.find(n => n.testId === `lx-picker-day-${id}`);
  open({ kind: 'date', value: '', today: '2026-11-13' });
  let nodes = render(view, env);
  assert.equal(nodes.filter(n => n.testId.startsWith('lx-picker-day-') && n.props.stateSelected === 1).length, 0, 'nothing chosen');
  assert.notEqual(day('2026-11-13').props.stateSelected, 1, 'today is not the choice');
  assert.equal(nodes.find(n => n.testId === 'lx-picker-done').props.enabled, 0, 'Done is inert until a day is picked');
  dispatch(nodes.find(n => n.testId === 'lx-picker-done').props.onClick);
  assert.equal(takePickerResults(), '[]', 'Done without a choice commits nothing');
  assert.equal(hasPicker(), true);
  dispatch(day('2026-11-20').props.onClick);
  nodes = render(view, env);
  assert.equal(day('2026-11-20').props.stateSelected, 1, 'the tapped day is chosen');
  assert.notEqual(nodes.find(n => n.testId === 'lx-picker-done').props.enabled, 0);
  dispatch(nodes.find(n => n.testId === 'lx-picker-done').props.onClick);
  assert.deepEqual(JSON.parse(takePickerResults()), [{ identity: '/i:0:0/i:0:0', value: '2026-11-20' }]);
});

test('on touch a date field with a value opens with that day chosen', () => {
  reset();
  open({ kind: 'date', value: '2026-10-29', today: '2026-11-13' });
  const nodes = render(() => h('view', {}), { pointer: 'coarse', language: 'en' });
  const chosen = nodes.filter(n => n.testId.startsWith('lx-picker-day-') && n.props.stateSelected === 1).map(n => n.testId);
  assert.deepEqual(chosen, ['lx-picker-day-2026-10-29']);
  dispatch(nodes.find(n => n.testId === 'lx-picker-done').props.onClick);
  assert.equal(JSON.parse(takePickerResults())[0].value, '2026-10-29');
});

test('an unbound date or time field sends no display text, only its width', () => {
  reset();
  const view = () => h('view', {}, h('input', { 'data-testid': 'at', type: 'time' }), h('input', { 'data-testid': 'on', type: 'date' }));
  const nodes = render(view, { hourCycle: 'h12', language: 'en-US' });
  for (const id of ['at', 'on']) {
    const props = nodes.find(n => n.testId === id).props;
    assert.equal(Object.hasOwn(props, 'displayValue'), false, id);
    assert.equal(Object.hasOwn(props, 'inputValue'), false, id);
    assert.ok(props.sizeText.length > 0, `${id} keeps its width`);
  }
});

test('the core reads an unbound field\'s own value in the committed page\'s clock and language', () => {
  reset(); publishHost(() => h('view', {}));
  const text = globalThis.__lingxiaFieldText;
  const view = () => h('input', { type: 'time' });
  render(view, { hourCycle: 'h12', language: 'en-US' });
  assert.equal(text('time', '13:05', {}), '1:05 PM');
  assert.equal(text('time', '09:30:15', {}), '9:30:15 AM');
  assert.equal(text('date', '2026-11-14', {}), 'Nov 14, 2026');
  render(view, { hourCycle: 'h23', language: 'zh-Hans-CN' });
  assert.equal(text('time', '13:05', {}), '13:05');
  assert.equal(text('date', '2026-11-14', {}), '2026年11月14日');
  // The core's own environment wins: it asks before the frame that carries a new one commits.
  assert.equal(text('time', '13:05', { language: 'en-US', hourCycle: 'h12' }), '1:05 PM');
  assert.equal(text('date', '2026-11-14', { language: 'en-US' }), 'Nov 14, 2026');
  // Text that does not parse, an unknown kind or a missing env comes back as given; nothing throws.
  assert.equal(text('date', 'soon', {}), 'soon');
  assert.equal(text('week', '2026-W46', {}), '2026-W46');
  assert.equal(text('time', 5, undefined), '');
  reset();
  assert.equal(text('time', '13:05'), '13:05');
});

test('a time field shows its ISO value in the user\'s clock and re-lowers when the clock or language changes', () => {
  reset();
  const view = () => h('view', {}, h('input', { 'data-testid': 'at', type: 'time', value: '13:05' }),
    h('input', { 'data-testid': 'on', type: 'date', value: '2026-11-14' }));
  const field = (nodes, id) => nodes.find(n => n.testId === id).props;
  let nodes = render(view, { hourCycle: 'h12', language: 'en-US' });
  assert.equal(field(nodes, 'at').inputValue, '13:05', 'the bound value stays ISO');
  assert.equal(field(nodes, 'at').displayValue, '1:05 PM');
  assert.equal(field(nodes, 'at').sizeText, '00:00 AM\n00:00 PM');
  assert.equal(field(nodes, 'on').inputValue, '2026-11-14', 'the bound date stays ISO');
  assert.equal(field(nodes, 'on').displayValue, 'Nov 14, 2026');
  assert.equal(field(nodes, 'on').sizeText.split('\n').length, 12, 'every month name sizes the field');
  nodes = render(view, { hourCycle: 'h23', language: 'en-US' });
  assert.equal(field(nodes, 'at').displayValue, '13:05');
  assert.equal(field(nodes, 'at').sizeText, '00:00');
  nodes = render(view, { hourCycle: 'h12', language: 'zh-Hans-CN' });
  assert.equal(field(nodes, 'at').displayValue, '1:05 下午');
  assert.equal(field(nodes, 'on').displayValue, '2026年11月14日');
  assert.equal(field(nodes, 'on').sizeText, '0000年0月00日\n0000年00月00日');
  assert.equal(field(nodes, 'at').sizeText, '00:00 上午\n00:00 下午');
  nodes = render(() => h('input', { 'data-testid': 'at', type: 'time', value: '09:30:15', step: 1 }), { hourCycle: 'h12', language: 'en-US' });
  assert.equal(field(nodes, 'at').displayValue, '9:30:15 AM');
  assert.equal(field(nodes, 'at').sizeText, '00:00:00 AM\n00:00:00 PM');
});

test('showPicker() asks a date editor for its picker and warns on an editor that has none', () => {
  reset();
  const warnings = [];
  globalThis.__lingxiaWarn = message => warnings.push(message);
  try {
    const cells = {};
    const view = () => {
      cells.day = useLocal('day', '2026-10-29'); cells.off = useLocal('off', '2026-10-29'); cells.name = useLocal('name', '');
      return h('view', {},
        h('input', { 'data-testid': 'day', type: 'date', bind: cells.day, 'aria-label': 'Day' }),
        h('input', { 'data-testid': 'off', type: 'date', bind: cells.off, disabled: true, 'aria-label': 'Off' }),
        h('input', { 'data-testid': 'name', bind: cells.name, 'aria-label': 'Name' }));
    };
    const request = (nodes, id) => nodes.find(n => n.testId === id).props.pickerRequest;
    let nodes = render(view, {});
    assert.equal(request(nodes, 'day'), undefined);
    cells.day.showPicker(); cells.off.showPicker(); cells.name.showPicker();
    nodes = render(view, {});
    assert.equal(request(nodes, 'day'), 1, 'the date editor carries the request');
    assert.equal(request(nodes, 'off'), undefined, 'a disabled editor opens nothing');
    assert.equal(request(nodes, 'name'), undefined);
    assert.deepEqual(warnings, ['UI_INPUT: showPicker() does nothing on <input>; only a date or time editor opens a picker']);
  } finally { delete globalThis.__lingxiaWarn; }
});
