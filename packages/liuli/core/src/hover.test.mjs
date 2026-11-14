import { test } from 'node:test';
import assert from 'node:assert/strict';
import { renderExample } from './testing/harness.mjs';

// Rows wash on hover as they do on focus; boxes firm their edge; a slider thumb grows.
const cases = [
  ['CheckboxInput/CheckboxInput', 'Basic', 'digest'],
  ['RadioGroup/RadioGroup', 'Delivery', 'delivery-standard'],
  ['Switch/Switch', 'InList', 'mentions'],
  ['Tabs/Tabs', 'Sections', 'project-overview'],
  ['Select/Select', 'Country', 'country'],
  ['Slider/Slider', 'Volume', 'volume'],
];

for (const [path, example, id] of cases) {
  test(`${example} (${path.split('/')[0]}): a pointer over it changes its paint`, async () => {
    const screen = await renderExample(path, example);
    const hover = screen.getByTestId(id).props.hoverStyle;
    assert.ok(hover && Object.keys(hover).length > 0, `${id} has no hover style`);
  });
}

test('a standalone switch has no hover wash', async () => {
  const basic = await renderExample('Switch/Switch', 'Basic');
  assert.equal(basic.getByTestId('sync').props.hoverStyle, undefined);
});
