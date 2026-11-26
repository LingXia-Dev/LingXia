import type { ComponentDoc } from '@liulidesign/core/internal/doc';

export const doc: ComponentDoc = {
  name: 'PieChart',
  category: 'charts',
  status: 'preview',
  since: '2026-11-14',
  summary: 'Parts of one whole, as a ring or a solid pie, listed with their amounts in a legend that selects',
  description:
    'PieChart shows how one total divides into a few parts: spending by category, storage by kind. ' +
    'Its `nameField`, `valueField` and `keyField` are fields of the rows\' own type (`Row`), so a misspelt one fails to compile. ' +
    'By default it is a ring with the total in its hole. The parts are named in a legend under it, with their amounts and shares, ' +
    'and a legend row or a tap on its slice chooses the part: the slice grows, the others step back, and the hole shows the chosen part; tapped again, the choice clears.',
  keywords: ['pie chart', 'donut', 'doughnut', 'ring', 'share', 'proportion', 'parts of a whole', 'chart', 'graph'],
  propsType: 'PieChartProps',
  recipe: { map: 'pieChartRecipe', parts: 'pieChartParts' },
  anatomy: [
    { part: 'plot', required: true, description: 'The box that centres the ring.' },
    { part: 'drawing', required: false, description: 'The slices, clockwise from the top with a constant gap between them; with `onChange` a tap chooses a slice. Hidden from assistive tech, which chooses in the legend.' },
    { part: 'centre', required: false, description: 'A donut\'s hole: the total under `totalLabel`, or the chosen part; announced when it changes.' },
  ],
  accessibility: {
    pattern: null,
    name: 'label',
    description: [],
    states: [{ state: 'pressed', from: 'value' }, { state: 'busy', from: 'isLoading' }],
    keyboard: [
      { keys: 'Tab', effect: 'Moves through the legend rows when they can be pressed' },
      { keys: 'Enter or Space', effect: 'Chooses the part, or lets it go' },
    ],
    target: { md: 36 },
    contrast: [
      { what: 'first mark colour', pair: ['chart-1', 'surface'], minimum: '3:1' },
      { what: 'second mark colour', pair: ['chart-2', 'surface'], minimum: '3:1' },
      { what: 'third mark colour', pair: ['chart-3', 'surface'], minimum: '3:1' },
      { what: 'fourth mark colour', pair: ['chart-4', 'surface'], minimum: '3:1' },
      { what: 'fifth mark colour', pair: ['chart-5', 'surface'], minimum: '3:1' },
      { what: 'second mark colour, straight on the page', pair: ['chart-2', 'canvas'], minimum: '3:1' },
      { what: 'third mark colour, straight on the page', pair: ['chart-3', 'canvas'], minimum: '3:1' },
      { what: 'title and figures', pair: ['text', 'surface'], minimum: '4.5:1' },
      { what: 'description, labels and legend', pair: ['text-muted', 'surface'], minimum: '4.5:1' },
      { what: 'a merged remainder slice', pair: ['line-strong', 'surface'], minimum: '3:1' },
      { what: 'a merged remainder slice, straight on the page', pair: ['line-strong', 'canvas'], minimum: '3:1' },
    ],
    notes: [
      'The legend is the reading: a list where each part is its name, amount and share. With `onChange` each row is a button that reports pressed.',
      'With `summarize`, a sentence with the number of parts, the total, and the largest and smallest part is read first.',
      'The hole is a status region: choosing a part announces its name and amount.',
      'Slices are told apart by colour and by their order, which the legend repeats from the top clockwise; the gap between slices keeps neighbours apart without relying on hue.',
      'A legend row is 36 px tall and as wide as the chart.',
      'The ring is one drawing, rebuilt only for a new `rows` array, encoding or chosen part; the legend is five elements per part.',
    ],
  },
  guidance: {
    do: [
      'Use it for two to five parts that add up to a meaningful whole.',
      'Set `otherLabel` when there may be more parts; the rest are gathered into one neutral slice.',
      'Order the rows by amount, largest first, unless the parts have an order of their own.',
      'Pass `formatShare` so the legend shows each part\'s share.',
      'At large text, a total too wide for the hole is written under the ring; keep `formatValue` short so it stays inside.',
    ],
    dont: [
      'Compare two pies; use a grouped BarChart.',
      'Use it for values that do not add up to a whole, or that can be negative.',
      'Hide the legend unless the parts are named elsewhere; slices carry no labels.',
    ],
  },
  examples: [
    { name: 'Spending', title: 'A ring with the total, selected through its legend' },
    { name: 'Solid', title: 'A solid pie' },
    { name: 'Merged', title: 'Many parts, the small ones gathered' },
  ],
  gaps: [
    { missing: 'Choosing a slice by pressing it.', needs: 'pointer coordinates on an element (`onPointer`)', today: 'The legend rows are the buttons.' },
    { missing: 'Slices that grow and turn when the data changes.', needs: 'animation of vector geometry', today: 'New data is drawn at once.' },
    { missing: 'Labels on or beside slices.', needs: 'a laid-out box\'s size, readable in render (`useBox`)', today: 'The legend names every part.' },
  ],
  related: ['Chart', 'BarChart'],
  gallery: {
    specimen: 'Specimen',
    tagline: 'Parts of a whole',
    order: 5,
    page: {
      lead: 'Use a pie chart to show how one total divides into a few parts.',
      hero: 'Spending',
      variants: [
        { name: 'Solid', title: 'A solid pie, when no total is needed' },
        { name: 'Merged', title: 'Eight parts: four keep a slice, the rest share a neutral one' },
      ],
      sizes: [{ name: 'Sizes', title: 'Small and large, the legend left out' }],
      statesTitle: 'Selected, loading, empty and error',
      statesNote: 'What the chart shows in each state. These are staged examples: nothing here is waiting or broken.',
      states: [
        { name: 'StateSelected', title: 'One chosen' },
        { name: 'StateWhole', title: 'One part' },
        { name: 'StateLoading', title: 'While loading' },
        { name: 'StateEmpty', title: 'With no data' },
        { name: 'StateError', title: 'After a failure' },
      ],
      statesLayout: 'stack',
    },
  },
};
