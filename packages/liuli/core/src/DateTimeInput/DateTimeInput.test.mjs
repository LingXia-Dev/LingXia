import { test } from 'node:test';
import assert from 'node:assert/strict';
import { renderExample, renderPage } from '../testing/harness.mjs';

test('Reminder: one group named by the label, with a named date and time holding the parts of one value', async () => {
  const screen = await renderExample('DateTimeInput/DateTimeInput', 'Reminder');
  const group = screen.getByTestId('remind');
  assert.deepEqual([group.props.role, group.props.accessibilityLabel], ['group', 'Remind me']);
  const [date, time] = [screen.getByTestId('remind-date'), screen.getByTestId('remind-time')];
  assert.deepEqual([date.props.accessibilityLabel, date.props.inputValue, time.props.accessibilityLabel, time.props.inputValue], ['Reminder date', '2026-11-20', 'Reminder time', '09:00']);
  screen.type(time, '14:30');
  screen.getByText('Set for 2026-11-20 at 14:30');
});

test('Required: the value is whole only once both parts are set; one part is kept while the other is empty', async () => {
  const screen = await renderExample('DateTimeInput/DateTimeInput', 'Required');
  const part = id => screen.getByTestId(`starts-${id}`);
  screen.type(part('date'), '2026-11-21');
  assert.deepEqual([part('date').props.inputValue, part('time').props.inputValue], ['2026-11-21', '']);
  screen.getByText('Choose a day and a time.');
  screen.type(part('time'), '08:15');
  assert.equal(screen.queryAll(node => node.text === 'Choose a day and a time.').length, 0);
});

test('Reminder: the pair wraps by base widths; each part grows to fill its line and has no least width of its own', async () => {
  const screen = await renderExample('DateTimeInput/DateTimeInput', 'Reminder');
  const group = screen.getByTestId('remind');
  assert.deepEqual([group.props.flexDirection, group.props.flexWrap], ['row', 'wrap']);
  // Each part is its own field column inside the group.
  const [date, time] = group.children;
  assert.deepEqual([date.props.flexBasis, date.props.grow, time.props.flexBasis, time.props.grow], [160, 2, 112, 1]);
  assert.ok(date.props.flexBasis + time.props.flexBasis + group.props.gap > 240, 'under 280 pt the time takes its own row');
  // The View keeps an editor as wide as its widest value; a least width of the part's own would push it past a narrow container.
  assert.deepEqual([date.props.minWidth, time.props.minWidth], [undefined, undefined]);
});

test('At large text: measured values that no longer fit side by side put the time under the date, each the whole width', async () => {
  const screen = await renderExample('DateTimeInput/DateTimeInput', 'Reminder');
  const group = () => screen.getByTestId('remind');
  const measured = () => group().children.at(-1).children;
  const editor = screen.getByTestId('remind-date').props.fontSize;
  assert.deepEqual(measured().map(node => [node.text, node.props.whiteSpace, node.props.fontSize]), [['Sep 30, 2026', 'nowrap', editor], ['12:00 PM', 'nowrap', editor]], 'set in the editors\' type');
  const set = (date, time) => {
    screen.layout(measured()[0], { width: date, height: 22 });
    screen.layout(measured()[1], { width: time, height: 22 });
  };
  screen.layout(group(), { width: 296, height: 48 });
  // Each part needs its value plus 34 pt of padding, border and gap and its 16 pt mark; 8 pt between the parts.
  set(100, 70);
  assert.deepEqual(group().children.slice(0, 2).map(part => part.props.flexBasis), [160, 112], 'at the standard size they share the line');
  set(130, 92);
  assert.deepEqual(group().children.slice(0, 2).map(part => part.props.flexBasis), ['100%', '100%'], 'at 130 % the time moves under the date');
});

test('At large text: the parts\' marks grow too, so values that fit beside each other at the standard size stack', async () => {
  const parts = async textScale => {
    const screen = await renderExample('DateTimeInput/DateTimeInput', 'Reminder', { env: { textScale } });
    const group = () => screen.getByTestId('remind');
    const measured = () => group().children.at(-1).children;
    screen.layout(group(), { width: 296, height: 48 });
    screen.layout(measured()[0], { width: 100, height: 22 });
    screen.layout(measured()[1], { width: 74, height: 22 });
    return group().children.slice(0, 2).map(part => part.props.flexBasis);
  };
  // 100 + 74 + 2 × (34 + 16) + 8 = 282 fits 296; with 24 pt marks it is 298.
  assert.deepEqual(await parts(1), [160, 112]);
  assert.deepEqual(await parts(1.5), ['100%', '100%']);
});

test('Clearable: a clear button named "Clear Remind me" empties the field, and is gone once it is empty', async () => {
  const screen = await renderExample('DateTimeInput/DateTimeInput', 'Clearable');
  const clear = screen.getByTestId('clearable-clear');
  assert.equal(clear.props.accessibilityLabel, 'Clear Remind me');
  screen.press(clear);
  assert.equal(screen.queryAll(node => node.testId === 'clearable-clear').length, 0, 'gone once empty');
});

test('a reset from outside empties both parts; a part the reader set alone is kept', async () => {
  const screen = await renderPage(`
    import { useLocal } from '@lingxia/view';
    import { DateTimeInput } from '@liulidesign/core/DateTimeInput';
    export function render() {
      const when = useLocal('when', '2026-11-20T09:00');
      return <page>
        <DateTimeInput label="Remind me" bind={when} data-testid="remind" />
        <button data-testid="reset" onClick={() => when.set('')}><text>Reset</text></button>
      </page>;
    }
  `);
  const part = id => screen.getByTestId(`remind-${id}`).props.inputValue;
  screen.type(screen.getByTestId('remind-time'), '14:30');
  screen.press(screen.getByTestId('reset'));
  assert.deepEqual([part('date'), part('time')], ['', ''], 'the old parts are not shown after a reset');
  screen.type(screen.getByTestId('remind-date'), '2026-11-22');
  assert.deepEqual([part('date'), part('time')], ['2026-11-22', '']);
});
