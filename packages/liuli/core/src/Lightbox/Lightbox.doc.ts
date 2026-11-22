import type { ComponentDoc } from '../doc';

export const doc: ComponentDoc = {
  name: 'Lightbox',
  category: 'media',
  status: 'preview',
  since: '2026-11-15',
  summary: 'Pictures one at a time, over the whole screen',
  description:
    'Lightbox covers the screen with an opaque dark surface and shows pictures whole, one at a time: a swipe or a dot moves between them, and a line of caption follows the one in view. ' +
    'It opens from a Thumbnail at the picture that was pressed, and closes from its button, Escape or back.',
  keywords: ['lightbox', 'image viewer', 'gallery', 'photo', 'fullscreen', 'zoom', 'preview'],
  propsType: 'LightboxProps',
  recipe: { map: 'lightboxRecipe', parts: 'lightboxParts' },
  anatomy: [
    { part: 'panel', required: true, description: 'The dialog primitive, covering the screen on the opaque media canvas.' },
    { part: 'bar', required: true, description: 'Where the viewer is, and the close button.' },
    { part: 'scroll', required: true, description: 'The pictures side by side, stopping on each.' },
    { part: 'image', required: true, description: 'A picture, shown whole.' },
    { part: 'caption', required: false, description: 'The caption of the picture in view.' },
  ],
  accessibility: {
    pattern: 'dialog',
    name: 'label',
    description: [],
    states: [],
    keyboard: [
      { keys: 'Escape', effect: 'Closes it' },
      { keys: 'Tab', effect: 'Between the close button and the dots' },
      { keys: 'Enter or Space on a dot', effect: 'Shows its picture' },
    ],
    target: { md: 44 },
    contrast: [
      { what: 'controls and caption', pair: ['on-media', 'media-canvas'], minimum: '4.5:1' },
    ],
    notes: [
      'A dialog named by `label`; assistive tech reads only the lightbox while it is open, and focus returns to what opened it.',
      'The pictures are a list read by their `alt`; the place ("2 of 4") is a status, said when it changes.',
      'A tap on the dark surface does not close it: a missed swipe would.',
    ],
  },
  guidance: {
    do: [
      'Open it at the picture that was pressed: set the bound page, then open.',
      'Write a caption only where it adds to the picture.',
    ],
    dont: [
      'Use it for a document or a video; it shows pictures.',
      'Put actions that change the picture inside it; it is for looking.',
    ],
  },
  examples: [
    { name: 'FromGrid', title: 'Opened from a grid of thumbnails' },
    { name: 'Single', title: 'One picture' },
  ],
  gaps: [],
  limits: [
    'No pinch to zoom and no pan: the View has no pinch gesture.',
    'No previous and next arrows; on a desktop the dots move between pictures.',
  ],
  playground: { props: { isOpen: false, label: 'Photos of the house' }, fixture: true, opener: 'View photos' },
  related: ['Thumbnail', 'Carousel', 'Dialog'],
  gallery: {
    specimen: 'Specimen',
    tagline: 'A picture, whole',
    order: 5,
    page: {
      lead: 'Use a lightbox to look at pictures whole.',
      hero: 'FromGrid',
      variants: [{ name: 'Single', title: 'One picture: no count and no dots' }],
    },
  },
};
