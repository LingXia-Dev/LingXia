import type { ComponentDoc } from '../doc';

export const doc: ComponentDoc = {
  name: 'Carousel',
  category: 'media',
  status: 'preview',
  since: '2026-11-15',
  summary: 'A few pages to swipe through, one at a time',
  description:
    'Carousel lays its children side by side in a scroll that stops on each one, and puts a PageControl with it: under the pages, or over their bottom edge for pictures. ' +
    'A swipe, a dot or the app\'s own button moves the page; `bind` shares it with the app. With `hasButtons` round buttons over the side edges move it too, ' +
    'and with `hasLoop` they go round from the last page to the first. With `itemWidth` it is a rail of cards, several in view, that stops on each.',
  keywords: ['carousel', 'slider', 'slideshow', 'gallery', 'pager', 'swipe', 'onboarding'],
  propsType: 'CarouselProps',
  recipe: { map: 'carouselRecipe', parts: 'carouselParts' },
  anatomy: [
    { part: 'root', required: true, description: 'The pages and their dots.' },
    { part: 'scroll', required: true, description: 'The sideways scroll that stops on every page.' },
    { part: 'track', required: true, description: 'The row of pages: a list named by `label`.' },
    { part: 'page', required: true, description: 'One child, as wide as the carousel or `itemWidth`: a list item.' },
    { part: 'button', required: false, description: 'With `hasButtons`: a round chip on the overlay surface over each side edge, gone at an end unless it loops.' },
    { part: 'dots', required: false, description: 'A PageControl bound to the same page.' },
  ],
  accessibility: {
    pattern: null,
    name: 'label',
    description: [],
    states: [],
    keyboard: [
      { keys: 'Tab', effect: 'Through the buttons and the dots, and through anything focusable on the page in view' },
      { keys: 'Enter or Space on a button', effect: 'Moves one page or one card' },
      { keys: 'Enter or Space on a dot', effect: 'Goes to its page' },
    ],
    target: {},
    contrast: [],
    notes: [
      'The pages are a list, so a screen reader says "2 of 4" on the page it lands on; assistive scrolling steps one page.',
      'The dots are tabs named by `pageLabel`. They are the way to page without a swipe.',
      'The buttons are named by `previousLabel` and `nextLabel`; a button with nowhere to go is removed, not dimmed.',
      'It never moves by itself.',
    ],
  },
  guidance: {
    do: [
      'Keep it to a handful of pages that are alike: photos of one thing, the steps of a welcome.',
      'Give every picture its `alt`.',
    ],
    dont: [
      'Hide things people must see behind a swipe; most never go past the first page.',
      'Put a carousel inside another sideways scroll.',
      'Move it on a timer.',
    ],
  },
  examples: [
    { name: 'Photos', title: 'Photos, with the dots over them' },
    { name: 'Welcome', title: 'Pages moved by a button too' },
    { name: 'Looping', title: 'Buttons over the edges, going round' },
    { name: 'Rail', title: 'A rail of cards' },
  ],
  gaps: [],
  limits: [
    'A swipe does not go round; only the buttons do.',
    'A rail\'s dots step one card, so most rails leave them out.',
  ],
  playground: { props: { label: 'Photos of the house', dots: 'over' }, fixture: true },
  related: ['PageControl', 'Lightbox', 'Thumbnail'],
  gallery: {
    specimen: 'Specimen',
    tagline: 'Swipe through a few',
    order: 3,
    page: {
      lead: 'Use a carousel for a few like things to swipe through.',
      hero: 'Photos',
      variants: [
        { name: 'Welcome', title: 'Content pages, with the page shared with a button' },
        { name: 'Looping', title: 'Buttons over the edges; past the last photo comes the first' },
        { name: 'Rail', title: 'A rail of 160-point cards that stops on each' },
      ],
    },
  },
};
