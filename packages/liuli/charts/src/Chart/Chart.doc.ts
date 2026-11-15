import type { ComponentDoc } from '@liulidesign/core/doc';

export const doc: ComponentDoc = {
  name: 'Chart',
  category: 'charts',
  status: 'preview',
  since: '2026-11-14',
  summary: 'The frame every chart sits in: heading, plot, legend, readout and the loading, empty and error states',
  description:
    'Chart is what BarChart, LineChart and PieChart share, and the frame for a plot of your own. ' +
    'It gives the plot a fixed height, names the chart for assistive tech, reads a summary sentence, and swaps the plot for a skeleton, a message or an error. ' +
    'ChartLegend and ChartReadout, from the same import, are the legend and the line that shows the chosen datum.',
  keywords: ['chart', 'frame', 'legend', 'readout', 'plot', 'graph', 'data visualisation', 'empty', 'loading'],
  propsType: 'ChartProps',
  otherPropsTypes: ['ChartLegendProps', 'ChartReadoutProps'],
  recipe: { map: 'chartRecipe', parts: 'chartParts' },
  anatomy: [
    { part: 'root', required: true, description: 'The group, named by `label`.' },
    { part: 'title', required: false, description: 'The heading; hidden from assistive tech when it repeats `label`.' },
    { part: 'description', required: false, description: 'One line under the title.' },
    { part: 'summary', required: false, description: 'The finding in sentences; read, not drawn.' },
    { part: 'readout', required: false, description: 'The chosen datum, on a line that keeps its height when nothing is chosen, and announced when it changes.' },
    { part: 'plot', required: true, description: 'The box the marks fill, at the height `size` sets; it holds the skeleton or the message in the other states.' },
    { part: 'axis', required: false, description: 'Category labels under the plot; hidden from assistive tech, since each mark is read with its category.' },
    { part: 'legend', required: false, description: 'The list of series or parts.' },
    { part: 'legendItem', required: false, description: 'One entry: a swatch of its mark and its name; a button when the legend can be pressed.' },
  ],
  accessibility: {
    pattern: null,
    name: 'label',
    description: [],
    states: [{ state: 'busy', from: 'isLoading' }, { state: 'pressed', from: 'ChartLegend selectedKey, or an entry that is shown' }],
    keyboard: [{ keys: 'Tab, Enter or Space', effect: 'Reach a legend entry that is a button, and press it' }],
    target: { md: 36 },
    contrast: [
      { what: 'first mark colour', pair: ['chart-1', 'surface'], minimum: '3:1' },
      { what: 'second mark colour', pair: ['chart-2', 'surface'], minimum: '3:1' },
      { what: 'third mark colour', pair: ['chart-3', 'surface'], minimum: '3:1' },
      { what: 'fourth mark colour', pair: ['chart-4', 'surface'], minimum: '3:1' },
      { what: 'fifth mark colour', pair: ['chart-5', 'surface'], minimum: '3:1' },
      { what: 'sixth mark colour', pair: ['hue-3', 'surface'], minimum: '3:1' },
      { what: 'seventh mark colour', pair: ['hue-5', 'surface'], minimum: '3:1' },
      { what: 'eighth mark colour', pair: ['hue-4', 'surface'], minimum: '3:1' },
      { what: 'second mark colour, straight on the page', pair: ['chart-2', 'canvas'], minimum: '3:1' },
      { what: 'third mark colour, straight on the page', pair: ['chart-3', 'canvas'], minimum: '3:1' },
      { what: 'title and figures', pair: ['text', 'surface'], minimum: '4.5:1' },
      { what: 'description, labels and legend', pair: ['text-muted', 'surface'], minimum: '4.5:1' },
      { what: 'a legend row under the pointer', pair: ['text', 'text/4'], over: 'surface', minimum: '4.5:1' },
    ],
    notes: [
      'A group named by `label`, read in order: the title when it differs from the name, the description, the summary, the readout, the marks, the legend.',
      'The readout is a status region: choosing a datum announces its category and figures.',
      'The legend is a list. An entry that hides a series reports pressed while the series is shown; an entry that selects reports pressed while chosen.',
      'An error is an alert and is announced when it appears. While loading the group is busy and the skeleton is hidden.',
      'Legend swatches repeat the mark: a square for bars and slices, a stroke with the line\'s dash for lines, so a series is never named by colour alone.',
      'An inline legend entry is 18 px tall; on touch, prefer the list variant (36 px rows) when entries can be pressed.',
    ],
  },
  guidance: {
    do: [
      'Use BarChart, LineChart or PieChart when one fits; use Chart to frame a plot they do not cover.',
      'Say what the chart shows in `label`, and its finding in `summary`.',
      'Hide a plot of your own from assistive tech and give its values as a legend list or a summary.',
      'Give every state its words: `emptyLabel` and `error` come from the app.',
    ],
    dont: [
      'Draw a border, a background or grid lines around the plot; the card it sits in is the frame.',
      'Put a legend on a chart with one series.',
      'Use colours outside the eight series colours for marks.',
    ],
  },
  examples: [
    { name: 'Budget', title: 'A plot of your own in the frame' },
    { name: 'Spending', title: 'Eight series colours' },
    { name: 'Legends', title: 'Legends for bars and for lines' },
    { name: 'Readout', title: 'The readout line' },
  ],
  gaps: [
    { missing: 'A value axis with ticks, and fitted labels for a plot of your own.', needs: 'nothing from the View', today: 'BarChart and LineChart fit their category labels and write the ends of their scale; a plot of your own passes its labels as `axis`.' },
    { missing: 'A tooltip that follows a finger or a pointer.', needs: 'nothing from the View', today: 'The readout line above the plot, which BarChart and LineChart fill as they are scrubbed.' },
    { missing: 'Marks that move between two sets of data.', needs: 'animation of vector geometry', today: 'Bars change height through layout transitions; lines and slices change at once.' },
  ],
  related: ['BarChart', 'LineChart', 'PieChart', 'Sparkline'],
  gallery: {
    specimen: 'Specimen',
    tagline: 'A chart\'s frame',
    order: 6,
    page: {
      lead: 'Use the chart frame around a plot of your own, so it reads and behaves like Liuli\'s charts.',
      hero: 'Budget',
      variants: [
        { name: 'Spending', title: 'Eight parts: the five chart colours, then three of the theme\'s hues' },
        { name: 'Legends', title: 'Squares for bars and slices, strokes for lines; the second can hide a series' },
        { name: 'Readout', title: 'The readout: the chosen datum in words and figures' },
      ],
      states: [
        { name: 'StateLoading', title: 'Loading' },
        { name: 'StateEmpty', title: 'Empty' },
        { name: 'StateError', title: 'Error' },
      ],
      statesLayout: 'stack',
    },
  },
};
