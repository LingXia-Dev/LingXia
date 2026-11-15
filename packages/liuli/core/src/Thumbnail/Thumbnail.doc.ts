import type { ComponentDoc } from '../doc';

export const doc: ComponentDoc = {
  name: 'Thumbnail',
  category: 'media',
  status: 'preview',
  since: '2026-11-15',
  summary: 'A small picture that stands for a bigger one',
  description:
    'Thumbnail crops a picture to a small rounded tile: square, 4:3 or 16:9. On its own it is an image; with `onClick` it is a button named by `alt` that opens the picture, usually in a Lightbox. ' +
    'A faint tint holds its place until the picture loads.',
  keywords: ['thumbnail', 'image', 'picture', 'photo', 'preview', 'tile'],
  propsType: 'ThumbnailProps',
  recipe: { map: 'thumbnailRecipe', parts: 'thumbnailParts' },
  anatomy: [
    { part: 'root', required: true, description: 'The rounded tile: a view, or a button with `onClick`.' },
    { part: 'image', required: true, description: 'The picture, cropped to fill the tile.' },
  ],
  accessibility: {
    pattern: null,
    name: 'alt',
    description: [],
    states: [],
    keyboard: [
      { keys: 'Tab', effect: 'Moves to a thumbnail that opens' },
      { keys: 'Enter or Space', effect: 'Opens it' },
    ],
    target: { sm: 48 },
    contrast: [{ what: 'selected ring', pair: ['accent', 'surface'], minimum: '3:1' }],
    notes: [
      'Without `onClick` it is an image read by `alt`; pass an empty `alt` when the name is already beside it, as in a row.',
      'With `onClick` it is a button named by `alt`; the selected one reports itself as selected.',
      'The selected ring is a shape, not only a colour.',
    ],
  },
  guidance: {
    do: [
      'Write `alt` for what the picture shows, not "photo" or its file name.',
      'Use `fill` in a grid so the columns set the width.',
    ],
    dont: [
      'Use a thumbnail for a person; that is an Avatar.',
      'Go below the small size for one that opens; it is the smallest a finger finds.',
    ],
  },
  examples: [
    { name: 'Grid', title: 'A grid of photos' },
    { name: 'InRows', title: 'At the start of a row' },
    { name: 'Sizes', title: 'Sizes and ratios' },
    { name: 'Strip', title: 'A strip with one selected' },
  ],
  gaps: [],
  limits: [
    'A picture that fails to load leaves the tint; there is no broken-image mark.',
  ],
  related: ['Lightbox', 'Carousel', 'Avatar'],
  gallery: {
    specimen: 'Specimen',
    tagline: 'A small picture',
    order: 4,
    page: {
      lead: 'Use a thumbnail to stand for a picture, a file or an album.',
      hero: 'Grid',
      variants: [{ name: 'Strip', title: 'A strip, with the one in view ringed' }],
      sizes: [{ name: 'Sizes', title: 'Small (48), medium (72), large (96), and large at 4:3' }],
      inContext: [{ name: 'InRows', title: 'At the start of a row' }],
    },
  },
};
