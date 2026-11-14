# @liulidesign/charts

Liuli's figures and small charts for GPU pages, as `@lingxia/view` TSX source, inspired by Astryx.
It follows `@liulidesign/core` in every convention: one import path per component, a recipe beside
each, every visible or spoken string a prop, colours from the app's theme roles.

Status: preview. Six components, each unit-tested in Node. None has been reviewed on a device yet.

## What it needs

- An lxapp with `"renderer": "gpu"`, `@lingxia/view` and `@liulidesign/core`.
- No stylesheet import. It uses Liuli core's type scale, which `tokens.css` repeats so that a page
  using charts alone has it.

## Use

```tsx
import { Sparkline } from '@liulidesign/charts/Sparkline';
import { Stat } from '@liulidesign/charts/Stat';

export function render() {
  return (
    <page className="p-4">
      <Stat
        label="Revenue"
        value="$48,250"
        delta="12.4%"
        deltaDirection="up"
        deltaLabel="Up 12.4% from last month"
        chart={<Sparkline values={[31, 34, 33, 38, 41, 44, 48]} label="Revenue by week: up from 31k to 48k" />}
      />
    </page>
  );
}
```

| Import | What |
| --- | --- |
| `@liulidesign/charts/Stat` | Stat: one figure with its label, how it changed, and a slot for a small chart |
| `@liulidesign/charts/Sparkline` | Sparkline: the shape of a series as a line, an area or bars, without axes |
| `@liulidesign/charts/BarChart` | BarChart: amounts by category on a zero line, grouped series, selection by pressing a bar |
| `@liulidesign/charts/LineChart` | LineChart: a number over time, as a line that fits its data or an area from zero, up to five series |
| `@liulidesign/charts/PieChart` | PieChart: parts of a whole as a ring or a pie, selected through its legend |
| `@liulidesign/charts/Chart` | Chart, ChartLegend and ChartReadout: the frame, legend and readout the charts share, for a plot of your own |
| `@liulidesign/charts/summary/en`, `/summary/zh` | `summarize`: a chart's facts as sentences for a screen reader, in English or Chinese |

Each also exports its class recipe at `@liulidesign/charts/<Name>/recipe`. The reference for each
component is in `docs/<Name>.md`; `catalog.json` holds the same for agents.

## Data and encoding

A chart takes rows and the names of the fields to draw, so the same rows feed a bar chart, a line
chart, a pie or a table:

```tsx
import { BarChart } from '@liulidesign/charts/BarChart';
import { summarize } from '@liulidesign/charts/summary/en';

const rows = [{ day: 'Mon', orders: 132 }, { day: 'Tue', orders: 148 }, { day: 'Wed', orders: 121 }];

<BarChart data={rows} x="day" y="orders" label="Orders by weekday" summarize={summarize}
  selectedKey={day.value} onSelect={day.set} />
```

`y` is one field, or a list of `{ field, label }` series. Rows are identified by `keyField` (by
default the category), which is what `selectedKey` holds. `formatValue` turns numbers into text.

## Conventions, beyond core's

- Liuli formats no numbers: figures and changes arrive as strings; chart values are numbers.
- Every chart takes a `label` that says its finding in words; it is all a screen reader gets.
- Numbers are set in tabular figures.
- A missing reading is `NaN` and draws as a gap, never as zero.
- Nothing is drawn that carries no data: no frames, grid lines or shadows.

## Layout of the package

As in core: `src/X/` holds `X.tsx`, `X.recipe.ts`, `X.doc.ts`, `X.examples.tsx` and `X.test.mjs`.
`src/internal/geometry.ts` turns values into path data; `src/internal/data.ts` is the row and
encoding model and the facts a summary is written from.

```sh
npm test            # unit tests: each example built by the View compiler and rendered in Node
npm run typecheck
```
