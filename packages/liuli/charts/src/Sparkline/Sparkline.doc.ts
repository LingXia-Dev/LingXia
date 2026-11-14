import type { ComponentDoc } from '@liulidesign/core/doc';

export const doc: ComponentDoc = {
  name: 'Sparkline',
  category: 'charts',
  status: 'preview',
  since: '2026-11-14',
  summary: 'A small chart without axes: the shape of a series as a line, an area or bars',
  description:
    'Sparkline shows how a number has moved, beside the number itself: in a Stat, a table cell or a list row. ' +
    'It has no axes, labels or interaction, so it answers "which way, and how steadily", never "how much". ' +
    'It is one drawing computed from `values`, and its `label` says in words what a sighted reader takes from the shape.',
  keywords: ['sparkline', 'trend', 'mini chart', 'inline chart', 'line', 'area', 'bar', 'series', 'history'],
  propsType: 'SparklineProps',
  recipe: { map: 'sparklineRecipe', parts: 'sparklineParts' },
  anatomy: [
    { part: 'root', required: true, description: 'The box, sized by `size` and the container, and the image assistive tech reads by `label`.' },
    { part: 'plot', required: false, description: 'The drawing, stretched to the box; absent when no value is finite.' },
  ],
  accessibility: {
    pattern: null,
    name: 'label',
    description: [],
    states: [],
    keyboard: [],
    contrast: [
      { what: 'series line and bars', pair: ['chart-1', 'surface'], minimum: '3:1' },
      { what: 'series line and bars, straight on the page', pair: ['chart-1', 'canvas'], minimum: '3:1' },
      { what: 'neutral line', pair: ['text-muted', 'surface'], minimum: '3:1' },
      { what: 'positive line', pair: ['success', 'surface'], minimum: '3:1' },
      { what: 'negative line', pair: ['danger', 'surface'], minimum: '3:1' },
    ],
    notes: [
      'One image named by `label`; the drawing itself is hidden. A screen reader gets nothing but the label, so it carries the finding: the direction, and the first and last values.',
      'It takes no focus and has no actions.',
      'The fill under an area is decoration and has no contrast minimum: the line above it carries the values.',
      'Positive and negative tones differ by colour alone, so the label, or the figure beside it, must say which it is.',
    ],
  },
  guidance: {
    do: [
      'Put it next to the figure it explains, and write the finding in `label`: "Revenue, last 7 days: up from 12k to 24k".',
      'Use a line for a trend, an area for an amount measured from zero, bars for separate counts or values on both sides of zero.',
      'Give sparklines that are compared with each other the same `domainMin` and `domainMax`.',
      'In a row, give it a width: `className="w-20"` or `grow`. It has no width of its own, so beside other content it would shrink to nothing.',
      'Pass `NaN` for a missing reading, so it shows as a gap.',
    ],
    dont: [
      'Use it where a value must be read off: it has no scale. Show the figure, or a chart with axes.',
      'Draw more than one series; put two sparklines side by side.',
      'Set a tone for looks; `positive` and `negative` are claims about the data.',
      'Fill missing readings with zero.',
    ],
  },
  examples: [
    { name: 'Trend', title: 'A trend, the latest value marked' },
    { name: 'Variants', title: 'Line, area and bars' },
    { name: 'Smooth', title: 'A rounded line that stays within its data' },
    { name: 'Tones', title: 'Tones' },
    { name: 'Sizes', title: 'Sizes' },
    { name: 'Gaps', title: 'Missing readings' },
    { name: 'InRows', title: 'In list rows' },
  ],
  gaps: [
    {
      missing: 'The value under a finger, and a moving marker.',
      needs: 'pointer coordinates on an element',
      today: 'The figure it explains sits beside it.',
    },
    {
      missing: 'Drawing in when it appears, and moving between two sets of values.',
      needs: 'animation of vector geometry',
      today: 'New values are drawn at once.',
    },
  ],
  related: ['Stat'],
  gallery: {
    specimen: 'Specimen',
    tagline: 'A trend at a glance',
    order: 2,
    page: {
      lead: 'Use a sparkline beside a figure to show which way it has been going.',
      hero: 'Trend',
      variants: [
        { name: 'Variants', title: 'Line, area and bars' },
        { name: 'Smooth', title: 'Smooth: rounded, never past its data' },
        { name: 'Tones', title: 'Tones; positive and negative are claims about the data' },
      ],
      sizes: [{ name: 'Sizes', title: 'Small for rows, medium by default, large on its own' }],
      states: [
        { name: 'StateMarked', title: 'Latest value marked' },
        { name: 'StateFlat', title: 'No change' },
        { name: 'StateSingle', title: 'One value' },
        { name: 'StateEmpty', title: 'No values' },
      ],
      inContext: [
        { name: 'Gaps', title: 'Missing readings stay gaps' },
        { name: 'InRows', title: 'In list rows, between the name and the figure' },
      ],
    },
  },
};
