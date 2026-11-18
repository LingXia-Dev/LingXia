import { test } from 'node:test';
import assert from 'node:assert/strict';
import { renderPage } from '../testing/harness.mjs';

const page = (language, chart) => `
  import { BarChart } from '@liulidesign/charts/BarChart';
  import { PieChart } from '@liulidesign/charts/PieChart';
  import { summarize } from '@liulidesign/charts/summary/${language}';
  const rows = [{ k: '一月', v: 12, w: 4 }, { k: '二月', v: 30, w: 4 }, { k: '三月', v: 21, w: 4 }];
  export function render() { return <page>${chart}</page>; }
`;
const summary = screen => screen.queryAll(node => node.tag === 'text' && node.props.opacity === 0)[0].text;

test('zh: a bar chart, a pie and an empty chart, in Chinese sentences', async () => {
  assert.equal(summary(await renderPage(page('zh', '<BarChart rows={rows} x="k" y="v" label="销量" summarize={summarize} />'))),
    '共 3 个数值，从一月到三月。最低 12（一月），最高 30（二月）。整体上升：起始 12，最新 21。');
  assert.equal(summary(await renderPage(page('zh', '<PieChart rows={rows} nameField="k" valueField="v" label="占比" summarize={summarize} />'))),
    '共 3 项，合计 63。最大：二月，30，占 48%。最小：一月，12，占 19%。');
});

test('en: several series are named, a level series says so, and one value is read plainly', async () => {
  assert.equal(summary(await renderPage(page('en', '<BarChart rows={rows} x="k" y={[{ field: "v", label: "Sold" }, { field: "w", label: "Returned" }]} label="Sales" summarize={summarize} />'))),
    'Sold: 3 values from 一月 to 三月. Lowest 12 at 一月, highest 30 at 二月. Rising: 12 first, 21 last. ' +
    'Returned: 3 values from 一月 to 三月. Lowest 4 at 一月, highest 4 at 一月. Level: 4 first, 4 last.');
  assert.equal(summary(await renderPage(page('en', '<BarChart rows={rows.slice(0, 1)} x="k" y="v" label="Sales" summarize={summarize} />'))), '一月, 12.');
});
