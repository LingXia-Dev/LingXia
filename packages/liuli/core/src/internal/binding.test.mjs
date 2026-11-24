import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';

const { components } = JSON.parse(readFileSync(new URL('../../catalog.json', import.meta.url), 'utf8'));
const propsOf = component => Object.fromEntries((component.props ?? []).map(prop => [prop.name, prop]));
const members = types => [...new Set(types.flatMap(type => type.split(' | ')).filter(type => type !== 'undefined'))].sort();
// A text field's cell is a LocalText; Calendar takes a cell of a day or of a span.
const cells = type => members([...type.matchAll(/Local(?:Text)?<([^<>]*)>/g)].map(match => match[1]));

test('state grammar: every component with `value` and `onChange` also takes `bind`, a cell of the value\'s own type, and none of them is required', () => {
  const stateful = components.filter(component => { const props = propsOf(component); return props.value && props.onChange; });
  assert.ok(stateful.length >= 25, `${stateful.length} components`);
  for (const component of stateful) {
    const { bind, value, onChange } = propsOf(component);
    assert.ok(bind, `${component.name}: no bind`);
    assert.deepEqual(cells(bind.type), members([value.type]), `${component.name}: bind holds what value carries`);
    assert.deepEqual([bind.required, value.required, onChange.required], [false, false, false], `${component.name}: one side or the other`);
  }
});

test('state grammar: no component names its state anything but `value`, `onChange` and `bind` (a `step` is an increment, `onSelect` an action)', () => {
  const others = /^(selected|checked|isPressed|isChecked|current|currentPage|page|activeKey|selectedKey|defaultValue|onPageChange|onStepChange|onPressedChange|onRangeChange|onValueChange)$/;
  const found = components.flatMap(component => (component.props ?? []).filter(prop => others.test(prop.name)).map(prop => `${component.name}.${prop.name}`));
  assert.deepEqual(found, []);
});

test('state grammar: with `value` and `onChange` the caller holds the state; Stepper, Table and Tour, whose examples bind, write through `onChange`', async () => {
  const { renderPage } = await import('../testing/harness.mjs');
  const { join } = await import('node:path');
  const screen = await renderPage(linked => `
import { useLocal } from '@lingxia/view';
import { Stepper } from ${JSON.stringify(join(linked, 'src/Stepper/Stepper'))};
import { Table } from ${JSON.stringify(join(linked, 'src/Table/Table'))};
import { Tour } from ${JSON.stringify(join(linked, 'src/Tour/Tour'))};
function Held() {
  const at = useLocal('at', 2);
  const row = useLocal<string | null>('row', null);
  const step = useLocal<number | null>('step', 0);
  return (
    <view>
      <view id="spot"><text>Spot</text></view>
      <Stepper label="Steps" steps={[{ key: 'a', label: 'Cart' }, { key: 'b', label: 'Pay' }, { key: 'c', label: 'Done' }]} value={at.value} onChange={at.set} data-testid="steps" />
      <Table label="Rows" columns={[{ key: 'name', header: 'Name' }]} rows={[{ name: 'Ada' }, { name: 'Lin' }]} keyField="name" value={row.value} onChange={row.set} data-testid="rows" />
      <Tour steps={[{ target: 'spot', targetLabel: 'The spot', title: 'One', text: 'First.' }, { target: 'spot', targetLabel: 'The spot', title: 'Two', text: 'Second.' }]} value={step.value} onChange={step.set} data-testid="tour" />
      <text data-testid="held">{\`\${at.value} \${row.value} \${step.value}\`}</text>
    </view>
  );
}
export function render() { return <page><Held /></page>; }
`);
  const held = () => screen.getByTestId('held').text ?? screen.getByTestId('held').children.map(child => child.text).join('');
  assert.equal(held(), '2 null 0');
  screen.press(screen.getByTestId('steps-a'));
  screen.press(screen.getByTestId('rows-Lin'));
  screen.press(screen.getByTestId('tour-next'));
  assert.equal(held(), '0 Lin 1');
});
