import type { ComponentDoc } from '../doc';

export const doc: ComponentDoc = {
  name: 'Icon',
  category: 'content',
  status: 'preview',
  since: '2026-11-14',
  summary: 'An icon by what it means, from a set an app or theme can replace',
  description:
    'Icon draws one icon in the colour of the text around it. Code asks for an icon by meaning, such as `close`, `search` or `chevron-end`, ' +
    'and the set in use decides its shape: the default set is 96 icons from Lucide, and `IconSetProvider` swaps in another set, or a single icon, for everything inside it. ' +
    'Icons that point along the reading direction turn round in right-to-left layouts.',
  keywords: ['icon', 'glyph', 'symbol', 'pictogram', 'lucide', 'icon set', 'vector'],
  propsType: 'IconProps',
  otherPropsTypes: ['IconSetProviderProps'],
  recipe: { map: 'iconRecipe', parts: 'iconParts' },
  anatomy: [
    { part: 'glyph', required: true, description: 'The drawing, on a 24-unit grid, in the current text colour.' },
    { part: 'box', required: false, description: 'A box around the drawing that carries `label`; present only with a label.' },
  ],
  accessibility: {
    pattern: null,
    name: 'label',
    description: [],
    states: [],
    keyboard: [],
    contrast: [
      { what: 'an icon that stands alone, in text colour', pair: ['text', 'surface'], minimum: '3:1' },
      { what: 'an icon that stands alone, muted', pair: ['text-muted', 'surface'], minimum: '3:1' },
    ],
    notes: [
      'Without `label` an icon is decoration: it is hidden from assistive tech, and the text or control beside it says what it means.',
      'With `label` it is an image named by the label. Use it only when the icon stands alone and carries meaning.',
      'An icon is never a control. A pressable icon is an IconButton, which has the press target and the name.',
      'An icon that carries meaning alone needs 3:1 against its background; `text-faint` does not reach it on every surface.',
    ],
  },
  guidance: {
    do: [
      'Ask by meaning: `name="delete"`, not the shape. A theme may draw it differently.',
      'Set the colour on the icon or its container with a text colour class; the icon follows it.',
      'Use `sm` beside footnote text, `md` beside body text and in controls, `lg` on its own.',
      'Draw an icon the set lacks with `icon`, on a 24-unit grid with a stroke of the same weight.',
    ],
    dont: [
      'Give an icon a label when text beside it says the same; it would be read twice.',
      'Use an icon alone for an action people rarely take; pair it with a word.',
      'Flip a directional icon yourself; the set marks which ones mirror.',
      'Scale an icon with a class to sizes between the three; the stroke would go soft.',
    ],
  },
  examples: [
    { name: 'Common', title: 'Icons by name' },
    { name: 'Sizes', title: 'Sizes' },
    { name: 'Colours', title: 'Colour from the text around it' },
    { name: 'Labelled', title: 'Read, or silent' },
    { name: 'Mirrored', title: 'Right to left' },
    { name: 'Custom', title: 'Your own icon, and a swapped one' },
    { name: 'Everything', title: 'The whole set' },
  ],
  gaps: [
    {
      missing: 'A stroke that keeps one weight at every size.',
      needs: 'nothing from the View; a decision on optical sizes',
      today: 'The stroke scales with the icon: 1.2 px at 16, 1.5 px at 20, 1.8 px at 24.',
    },
    {
      missing: 'Filled and two-tone variants of a name.',
      needs: 'a second set and a variant on the name',
      today: 'One outline per name; an app passes its own filled icon with `icon`.',
    },
  ],
  related: ['Button', 'Item'],
  gallery: {
    specimen: 'Specimen',
    tagline: 'Icons by meaning',
    order: 6,
    page: {
      lead: 'Use an icon to make an action or a kind of thing quick to spot, beside the word that names it.',
      hero: 'Common',
      variants: [
        { name: 'Colours', title: 'An icon takes the colour of the text around it' },
        { name: 'Labelled', title: 'Alone, it needs a label; beside text that says the same, it stays silent' },
        { name: 'Mirrored', title: 'Left to right above, right to left below: the first three turn round, the search glass does not' },
        { name: 'Custom', title: 'An icon of your own, the default close, then a set that redraws close and keeps the rest' },
      ],
      sizes: [{ name: 'Sizes', title: '16, 20 and 24' }],
      inContext: [{ name: 'Everything', title: 'The whole set, by name' }],
    },
  },
};
