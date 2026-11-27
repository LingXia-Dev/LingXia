# @liulidesign/charts

Liuli's figures and small charts for GPU pages, as `@lingxia/view` TSX source, inspired by Astryx.
It follows `@liulidesign/core` in every convention: one import path per component, a recipe beside
each, every visible or spoken string a prop, colours from the app's theme roles.

Status: preview. Each component is unit-tested in Node and has a page in the gallery (`../gallery`).

## What it needs

- A GPU View lxapp (its pages export `render`) with `@lingxia/view` and `@liulidesign/core`.
- No stylesheet import. It uses Liuli core's type scale, which the build loads from core, its peer.

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

<!-- components:start -->
6 components, each at its own import path, and all of their exports from `@liulidesign/charts`:

| Import | Exports | What it is |
| --- | --- | --- |
| `@liulidesign/charts/Stat` | Stat | One figure with its label, how it changed, and an optional small chart |
| `@liulidesign/charts/Sparkline` | Sparkline | A small chart without axes: the shape of a series as a line, an area or bars |
| `@liulidesign/charts/BarChart` | BarChart | Amounts by category, as bars on a zero line, with selection and grouped series |
| `@liulidesign/charts/LineChart` | LineChart | A number over time, as a line that fits its data or an area from zero, for up to five series |
| `@liulidesign/charts/PieChart` | PieChart | Parts of one whole, as a ring or a solid pie, listed with their amounts in a legend that selects |
| `@liulidesign/charts/Chart` | Chart, ChartLegend, ChartReadout | The frame every chart sits in: heading, plot, legend, readout and the loading, empty and error states |
<!-- components:end -->

| Import | What it is |
| --- | --- |
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

<BarChart rows={rows} x="day" y="orders" label="Orders by weekday" summarize={summarize}
  value={day.value} onChange={day.set} />
```

`y` is one field, or a list of `{ field, label }` series. Rows are identified by `keyField` (by
default the category), which is what `value` holds. `formatValue` turns numbers into text.

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
