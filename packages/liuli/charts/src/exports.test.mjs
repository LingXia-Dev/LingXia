import { test } from 'node:test';
import assert from 'node:assert/strict';
import { renderPage } from './testing/harness.mjs';

test('the package root gives every chart', async () => {
  const screen = await renderPage(`
    import { BarChart, LineChart, PieChart, Sparkline, Stat } from '@liulidesign/charts';
    const rows = [{ k: 'a', v: 1 }, { k: 'b', v: 2 }];
    export function render() {
      return (
        <page>
          <BarChart rows={rows} x="k" y="v" label="Bars" />
          <LineChart rows={rows} x="k" y="v" label="Line" />
          <PieChart rows={rows} nameField="k" valueField="v" label="Pie" />
          <Sparkline values={[1, 2]} label="Spark" />
          <Stat label="Total" value="3" />
        </page>
      );
    }`);
  for (const label of ['Bars', 'Line', 'Pie', 'Spark']) assert.ok(screen.queryAll(node => node.props.accessibilityLabel === label).length > 0, label);
});
