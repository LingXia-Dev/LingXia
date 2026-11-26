import { test } from 'node:test';
import assert from 'node:assert/strict';
import { textOf } from '@lingxia/view/testing';
import { renderExample, renderPage } from '../testing/harness.mjs';

const thumb = screen => screen.queryAll(node => node.props.widthPercent !== undefined)[0];

test('Units: a named radio group; a tap checks a segment, moves the thumb and writes the cell', async () => {
  const screen = await renderExample('SegmentedControl/SegmentedControl', 'Units');
  const group = screen.getByTestId('units');
  assert.equal(group.props.role, 'radiogroup');
  assert.equal(group.props.accessibilityLabel, 'Units');
  const km = screen.getByTestId('units-km');
  assert.equal(km.tag, 'button');
  assert.equal(km.props.role, 'radio');
  assert.equal(km.props.accessibilityLabel, 'Kilometres');
  assert.equal(km.props.stateChecked, 1);
  assert.equal(Math.round(thumb(screen).props.widthPercent), 50);
  assert.deepEqual(JSON.parse(thumb(screen).props.transform).translate, ['0%', 0]);
  screen.press(screen.getByTestId('units-mi'));
  assert.equal(screen.getByTestId('units-mi').props.stateChecked, 1);
  assert.equal(screen.getByTestId('units-km').props.stateChecked, 0);
  assert.deepEqual(JSON.parse(thumb(screen).props.transform).translate, ['100%', 0]);
  assert.equal(textOf(screen.getByTestId('distance')), '7.7 mi');
});

test('Appearance: value and onChange drive it; the thumb is a third wide', async () => {
  const screen = await renderExample('SegmentedControl/SegmentedControl', 'Appearance');
  assert.equal(screen.getByTestId('appearance-system').props.stateChecked, 1);
  screen.press(screen.getByTestId('appearance-dark'));
  assert.equal(screen.getByTestId('appearance-dark').props.stateChecked, 1);
  assert.equal(Math.round(thumb(screen).props.widthPercent), 33);
  assert.deepEqual(JSON.parse(thumb(screen).props.transform).translate, ['200%', 0]);
});

test('StateDisabled: a disabled segment stays in the row but cannot be chosen', async () => {
  const screen = await renderExample('SegmentedControl/SegmentedControl', 'StateDisabled');
  const max = screen.getByTestId('quality-max');
  assert.equal(max.props.enabled, 0);
  assert.equal(max.props.onClick, undefined);
});

test('Large text: a long label widens its segment and the thumb follows; a row that cannot hold the labels stacks', async () => {
  const screen = await renderExample('SegmentedControl/SegmentedControl', 'Units');
  const group = () => screen.getByTestId('units');
  const segment = value => screen.getByTestId(`units-${value}`);
  const box = () => screen.queryAll(node => node.props.background === 'surface-raised')[0];
  assert.equal(segment('km').children[0].children[0].props.whiteSpace, 'nowrap', 'a label in a row is never cut');
  screen.layout(group(), { width: 200, height: 36 });
  screen.layout(segment('km').children[0], { width: 100, height: 22 });
  screen.layout(segment('mi').children[0], { width: 40, height: 22 });
  screen.layout(segment('km'), { width: 124, height: 36 });
  screen.layout(segment('mi'), { width: 76, height: 36 });
  assert.deepEqual([box().props.width, JSON.parse(box().props.transform).translate], [124, [0, 0]]);
  screen.press(segment('mi'));
  assert.deepEqual([box().props.width, JSON.parse(box().props.transform).translate], [76, [124, 0]], 'after Kilometres');
  screen.layout(group(), { width: 180, height: 36 });
  assert.equal(group().props.flexDirection, 'column', 'Kilometres and Miles with 12 pt each side need 188');
  assert.equal(Math.round(box().props.heightPercent), 50);
  assert.deepEqual(JSON.parse(box().props.transform).translate, [0, '100%']);
  assert.equal(segment('km').props.grow, undefined);
  screen.layout(group(), { width: 300, height: 72 });
  assert.equal(group().props.flexDirection, 'row', 'room again, a row again');
});

test('Sizes: a segment pads its label 8 pt each side when small and 12 pt at the default size, so a control sized to its labels never runs them together', async () => {
  const screen = await renderExample('SegmentedControl/SegmentedControl', 'Sizes');
  const groups = screen.queryAll(node => node.props.role === 'radiogroup');
  const pads = groups.map(group => group.children.filter(child => child.props.role === 'radio').map(segment => segment.children[0]).map(content => [content.props.paddingLeft ?? content.props.paddingInlineStart, content.props.paddingRight ?? content.props.paddingInlineEnd]));
  assert.deepEqual(pads.map(each => [...new Set(each.map(String))]), [['8,8'], ['12,12']]);
});

test('Icons: an icon-only segment keeps its label as its name and is chosen as any other', async () => {
  const screen = await renderExample('SegmentedControl/SegmentedControl', 'Icons');
  const list = screen.getByTestId('view-list');
  assert.equal(list.props.accessibilityLabel, 'List');
  screen.press(list);
  assert.equal(screen.getByTestId('view-list').props.stateChecked, 1);
});

test('a value longer than the 64 characters a box name holds still draws its segment', async () => {
  const long = 'v'.repeat(80);
  const screen = await renderPage(`
    import { useLocal } from '@lingxia/view';
    import { SegmentedControl } from '@liulidesign/core/SegmentedControl';
    export function render() {
      const value = useLocal('value', '${long}');
      return <page><SegmentedControl label="Mode" bind={value} items={[{ value: '${long}', label: 'Long' }, { value: 'short', label: 'Short' }]} data-testid="mode" /></page>;
    }
  `);
  assert.equal(screen.getByTestId(`mode-${long}`).props.stateChecked, 1);
  screen.press(screen.getByTestId('mode-short'));
  assert.equal(screen.getByTestId('mode-short').props.stateChecked, 1);
});

test('with no items there is no thumb to draw', async () => {
  const screen = await renderPage(`
    import { SegmentedControl } from '@liulidesign/core/SegmentedControl';
    export function render() { return <page><SegmentedControl label="Mode" value="" onChange={() => {}} items={[]} data-testid="mode" /></page>; }
  `);
  assert.equal(thumb(screen), undefined);
});
