import { test } from 'node:test';
import assert from 'node:assert/strict';
import { renderPage } from '../testing/harness.mjs';

const page = chart => `
  import { BarChart } from '@liulidesign/charts/BarChart';
  import { LineChart } from '@liulidesign/charts/LineChart';
  import { PieChart } from '@liulidesign/charts/PieChart';
  const rows = [{ k: '一月', v: 12, w: 4 }, { k: '二月', v: 30, w: 4 }, { k: '三月', v: 21, w: 4 }];
  export function render() { return <page>${chart}</page>; }
`;
const summary = screen => screen.queryAll(node => node.tag === 'text' && node.props.opacity === 0)[0].text;

test('zh: a bar chart, a line, a pie and an empty chart, in Chinese sentences; only the line has a trend', async () => {
  assert.equal(summary(await renderPage(page('<BarChart rows={rows} x="k" y="v" label="销量" />'), { env: { language: 'zh-CN' } })),
    '共 3 个数值，从一月到三月。最低 12（一月），最高 30（二月）。');
  assert.equal(summary(await renderPage(page('<LineChart rows={rows} x="k" y="v" label="销量" />'), { env: { language: 'zh-CN' } })),
    '共 3 个数值，从一月到三月。最低 12（一月），最高 30（二月）。整体上升：起始 12，最新 21。');
  assert.equal(summary(await renderPage(page('<PieChart rows={rows} nameField="k" valueField="v" label="占比" />'), { env: { language: 'zh-CN' } })),
    '共 3 项，合计 63。最大：二月，30，占 48%。最小：一月，12，占 19%。');
});

test('en: several series are named, a level line says so, bars have no trend, and one value is read plainly', async () => {
  assert.equal(summary(await renderPage(page('<BarChart rows={rows} x="k" y={[{ field: "v", label: "Sold" }, { field: "w", label: "Returned" }]} label="Sales" />'), { env: { language: 'en' } })),
    'Sold: 3 values from 一月 to 三月. Lowest 12 at 一月, highest 30 at 二月. ' +
    'Returned: 3 values from 一月 to 三月. Lowest 4 at 一月, highest 4 at 一月.');
  assert.equal(summary(await renderPage(page('<LineChart rows={rows} x="k" y={[{ field: "v", label: "Sold" }, { field: "w", label: "Returned" }]} label="Sales" />'), { env: { language: 'en' } })),
    'Sold: 3 values from 一月 to 三月. Lowest 12 at 一月, highest 30 at 二月. Rising: 12 first, 21 last. ' +
    'Returned: 3 values from 一月 to 三月. Lowest 4 at 一月, highest 4 at 一月. Level: 4 first, 4 last.');
  assert.equal(summary(await renderPage(page('<BarChart rows={rows.slice(0, 1)} x="k" y="v" label="Sales" />'), { env: { language: 'en' } })), '一月, 12.');
});
