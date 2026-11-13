import type { ComponentDoc } from '../doc';

export const doc: ComponentDoc = {
  name: 'Skeleton',
  category: 'feedback',
  status: 'preview',
  summary: 'Quiet shapes that stand in for content while it loads',
  description:
    'Skeleton draws faint lines, blocks and circles in the shape of content that is on its way, so the page keeps its layout ' +
    'and does not jump when the content lands. SkeletonGroup, from the same import, wraps the shapes for one piece of content ' +
    'and tells assistive tech what is loading. A soft glint crosses each shape; it stops under reduced motion.',
  keywords: ['skeleton', 'placeholder', 'loading', 'shimmer', 'ghost', 'content loader', 'busy'],
  propsType: 'SkeletonProps',
  otherPropsTypes: ['SkeletonGroupProps'],
  recipe: { map: 'skeletonRecipe', parts: 'skeletonParts' },
  anatomy: [
    { part: 'group', required: false, description: 'SkeletonGroup: a status region named by `label`, marked busy.' },
    { part: 'lines', required: false, description: 'With `shape` text, the column of lines; the last of several is shorter.' },
    { part: 'bone', required: true, description: 'One shape: a faint fill in the text colour.' },
    { part: 'glint', required: true, description: 'A soft band of the surface colour that crosses the shape, then rests.' },
  ],
  accessibility: {
    pattern: null,
    name: null,
    description: [],
    states: [{ state: 'busy', from: 'SkeletonGroup, always' }],
    keyboard: [],
    contrast: [],
    notes: [
      'Each shape is hidden from assistive tech. SkeletonGroup is a status region named by `label` and marked busy, so the loading is said once, not per shape.',
      'The shapes are decorative and need no contrast; the content that replaces them meets its own.',
      'With reduced motion the glint stays out of view and the shapes are still.',
    ],
  },
  guidance: {
    do: [
      'Shape the skeleton like the content that will arrive: the same rows, the same picture size.',
      'Wrap the skeletons for one piece of content in a SkeletonGroup named by what is loading.',
      'Swap the whole group for the content at once.',
    ],
    dont: [
      'Use it for a wait inside a control; use Button\'s loading state or a Spinner.',
      'Keep it up when the answer is "nothing"; show an EmptyState.',
      'Skeleton text the person must read; a label or a value that is known now is drawn now.',
    ],
  },
  examples: [
    { name: 'ListLoading', title: 'A list loading' },
    { name: 'CardLoading', title: 'A card loading' },
  ],
  gaps: [],
  related: ['EmptyState', 'ProgressBar'],
  gallery: {
    specimen: 'Specimen',
    tagline: 'Content on its way',
    order: 5,
    page: {
      lead: 'Use skeletons where content is about to appear, shaped like it, so the page keeps its layout while it loads.',
      hero: 'ListLoading',
      variants: [{ name: 'Shapes', title: 'Lines, a circle and a block' }],
      inContext: [{ name: 'CardLoading', title: 'A card loading' }],
    },
  },
};
