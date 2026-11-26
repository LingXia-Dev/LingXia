import { test } from 'node:test';
import assert from 'node:assert/strict';
import { textOf } from '@lingxia/view/testing';
import { described, renderExample, renderPage } from '../testing/harness.mjs';

const fine = { env: { pointer: 'fine' } };
const panel = (screen, id) => screen.queryAll(node => node.testId === `${id}-panel`);

test('Stay: the field is a button named by its label, with the span as the app formats it read after the name', async () => {
  const screen = await renderExample('DateRangeInput/DateRangeInput', 'Stay');
  const field = screen.getByTestId('stay');
  assert.equal(field.tag, 'button');
  assert.equal(field.props.accessibilityLabel, 'Stay');
  assert.equal(textOf(field), '18 Nov – 24 Nov');
  assert.equal(field.children[0].props.fontVariantNumeric, 'tabular-nums');
  assert.deepEqual(described(screen, field), ['18 Nov – 24 Nov', 'Check-in from 3 pm, check-out by 11 am.']);
  assert.equal(panel(screen, 'stay').length, 0, 'closed, no calendar is rendered');
});

test('Stay, on touch: a sheet titled by the label; the first day starts a span and stays open, the second ends it and closes', async () => {
  const screen = await renderExample('DateRangeInput/DateRangeInput', 'Stay');
  const field = () => screen.getByTestId('stay');
  screen.press(field());
  const [sheet] = panel(screen, 'stay');
  assert.equal(sheet.props.accessibilityLabel, 'Stay');
  assert.equal(screen.queryAll(node => node.props.role === 'heading').map(textOf).includes('Stay'), true);
  const day = date => screen.getByTestId(`stay-calendar-${date}`);
  assert.equal(day('2026-11-21').props.stateSelected, 1, 'the span is in the calendar');
  assert.equal(day('2026-11-10').props.onClick, undefined, 'before the minimum');
  screen.press(day('2026-11-20'));
  assert.equal(panel(screen, 'stay').length, 1, 'still open after the first day');
  assert.equal(textOf(field()), '20 Nov – ', 'a span in progress shows an open end');
  screen.press(day('2026-11-27'));
  assert.equal(panel(screen, 'stay').length, 0);
  assert.equal(textOf(field()), '20 Nov – 27 Nov');
  screen.press(field());
  screen.press(screen.getByTestId('stay-close'));
  assert.equal(panel(screen, 'stay').length, 0);
  assert.equal(textOf(field()), '20 Nov – 27 Nov', 'closing keeps the span');
});

test('Stay, with a mouse: the calendar is in a panel under the field', async () => {
  const screen = await renderExample('DateRangeInput/DateRangeInput', 'Stay', fine);
  screen.press(screen.getByTestId('stay'));
  const [popover] = panel(screen, 'stay');
  assert.deepEqual([popover.props.width, popover.props.marginTop, popover.props.background], [312, 4, 'surface-overlay']);
  assert.equal(screen.queryAll(node => node.props.role === 'heading').length, 0, 'no sheet title');
  screen.press(screen.getByTestId('stay-calendar-2026-11-19'));
  screen.press(screen.getByTestId('stay-calendar-2026-11-16'));
  assert.equal(panel(screen, 'stay').length, 0);
  assert.equal(textOf(screen.getByTestId('stay')), '16 Nov – 19 Nov', 'the earlier day first');
});

test('Empty and states: the placeholder is muted; an error marks the edge and is read; disabled takes no press', async () => {
  const empty = await renderExample('DateRangeInput/DateRangeInput', 'Empty');
  const field = empty.getByTestId('leave');
  assert.equal(textOf(field), 'First and last day');
  assert.equal(field.children[0].props.color, 'text-muted');
  const invalid = await renderExample('DateRangeInput/DateRangeInput', 'StateInvalid');
  assert.equal(invalid.getByTestId('invalid').props.borderColor, 'danger');
  assert.deepEqual(described(invalid, invalid.getByTestId('invalid')), ['1 Dec – 5 Dec', 'These nights are taken.']);
  const disabled = (await renderExample('DateRangeInput/DateRangeInput', 'StateDisabled')).getByTestId('disabled');
  assert.equal(disabled.props.onClick, undefined);
});

test('firstWeekday: the calendar it opens starts its week where a standalone Calendar would', async () => {
  const screen = await renderPage(`
    import { DateRangeInput } from '@liulidesign/core/DateRangeInput';
    export function render() {
      return <page><DateRangeInput label="Term" value={{ start: null, end: null }} onChange={() => {}} format={day => day} today="2026-11-15" firstWeekday={1} data-testid="term" /></page>;
    }
  `, fine);
  screen.press(screen.getByTestId('term'));
  assert.equal(textOf(screen.queryAll(node => node.props.role === 'grid')[0].children[0]), 'MTWTFSS');
});

test('Clearable: a clear button named "Clear Stay" empties the field, and is gone once it is empty', async () => {
  const screen = await renderExample('DateRangeInput/DateRangeInput', 'Clearable');
  const clear = screen.getByTestId('clearable-clear');
  assert.equal(clear.props.accessibilityLabel, 'Clear Stay');
  screen.press(clear);
  assert.equal(screen.queryAll(node => node.testId === 'clearable-clear').length, 0, 'gone once empty');
});

test('Presets: named spans over the calendar; the one held reports pressed, and another pressed fills the field and closes the calendar', async () => {
  const screen = await renderExample('DateRangeInput/DateRangeInput', 'Presets');
  screen.press(screen.getByTestId('period'));
  const group = screen.getByTestId('period-presets');
  assert.equal(group.props.role, 'group');
  assert.equal(group.props.accessibilityLabel, 'Quick ranges');
  const preset = name => screen.getByTestId(`period-preset-${name}`);
  assert.equal(preset('Last 7 days').props.statePressed, 1, 'the field holds this span');
  assert.equal(preset('Last month').props.statePressed, 0);
  screen.press(preset('Last month'));
  assert.equal(panel(screen, 'period').length, 0, 'a finished span closes the sheet');
  assert.equal(textOf(screen.getByTestId('period')), '1 Oct – 31 Oct');
  screen.press(screen.getByTestId('period'));
  assert.deepEqual(['Last 7 days', 'Last month'].map(name => preset(name).props.statePressed), [0, 1], 'the held span reports pressed');
});

