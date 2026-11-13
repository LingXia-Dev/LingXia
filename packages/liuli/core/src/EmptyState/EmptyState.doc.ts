import type { ComponentDoc } from '../doc';

export const doc: ComponentDoc = {
  name: 'EmptyState',
  category: 'feedback',
  status: 'preview',
  summary: 'What a page or list shows when it has nothing in it yet, and the way forward',
  description:
    'EmptyState fills the space a list, page or panel would use when there is nothing to show: before the first item, after ' +
    'a search finds nothing, or when content is out of reach. It is a title, an optional description and actions, and an ' +
    'optional picture the app supplies. It draws no fill, border or illustration of its own.',
  keywords: ['empty state', 'empty', 'blank slate', 'no results', 'nothing here', 'zero state', 'first run', 'placeholder'],
  propsType: 'EmptyStateProps',
  recipe: { map: 'emptyStateRecipe', parts: 'emptyStateParts' },
  anatomy: [
    { part: 'root', required: true, description: 'A centred column with no fill or border, so it sits in whatever holds it.' },
    { part: 'media', required: false, description: '`media`: the app\'s icon or illustration, in the faint text colour; hidden from assistive tech.' },
    { part: 'title', required: true, description: 'A heading at `headingLevel`.' },
    { part: 'description', required: false, description: 'What to do about it.' },
    { part: 'actions', required: false, description: '`actions`: one or two Buttons, wrapping onto a second line on a narrow screen.' },
  ],
  accessibility: {
    pattern: null,
    name: null,
    description: [],
    states: [],
    keyboard: [],
    contrast: [
      { what: 'title', pair: ['text', 'surface'], minimum: '4.5:1' },
      { what: 'title on the page', pair: ['text', 'canvas'], minimum: '4.5:1' },
      { what: 'description', pair: ['text-muted', 'surface'], minimum: '4.5:1' },
      { what: 'description on the page', pair: ['text-muted', 'canvas'], minimum: '4.5:1' },
    ],
    notes: [
      'The title is a heading, so a screen reader can jump to it; set `headingLevel` to fit the page outline.',
      'The picture is decorative and hidden; say everything that matters in the title and description.',
      'It is not announced when it appears. After a search, the app reports the count in its own status line if it needs to be spoken.',
    ],
  },
  guidance: {
    do: [
      'Say what is empty and why, then what to do: "No notebooks yet", "Notebooks keep related notes together".',
      'Offer the action that fills it, and make it the first button.',
      'Use `sm` inside a card or a list section, and `md` for a whole page or panel.',
    ],
    dont: [
      'Show an empty state while content is loading; show Skeleton until the answer is known.',
      'Use it for an error the person can fix in place; use a Banner with the fix.',
      'Put more than two actions in it.',
    ],
  },
  examples: [
    { name: 'FirstRun', title: 'First run, with the action that starts it' },
    { name: 'NoResults', title: 'No results, with a way back' },
    { name: 'InCard', title: 'Small, inside a card' },
  ],
  gaps: [],
  related: ['Skeleton', 'Banner', 'Button'],
  gallery: {
    specimen: 'Specimen',
    tagline: 'Nothing here yet',
    order: 4,
    page: {
      lead: 'Use an empty state when a list or page has nothing to show yet, to say why and offer the way forward.',
      hero: 'FirstRun',
      variants: [{ name: 'NoResults', title: 'No results, with a way back' }],
      sizes: [{ name: 'Sizes', title: 'Small for a card or list, medium for a page' }],
      states: [
        { name: 'StateTitleOnly', title: 'A title only' },
        { name: 'StateWithPicture', title: 'With a picture' },
        { name: 'StateWithAction', title: 'With an action' },
      ],
      inContext: [{ name: 'InCard', title: 'Inside a card' }],
    },
  },
};
