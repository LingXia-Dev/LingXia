import type { ComponentDoc } from '../doc';

export const doc: ComponentDoc = {
  name: 'Tour',
  category: 'overlays',
  status: 'preview',
  since: '2026-11-15',
  summary: 'A few callouts, one at a time, each pointing at something',
  description:
    'Tour shows one callout at a time beside an element of the page: a title, a sentence or two, where the tour is, and buttons to go on, go back or skip. ' +
    'Each step names its target by `id`; the callout sits under, over or beside it and flips for room. The app holds which step is in view, so it can start, end and remember the tour. ' +
    'The page is not dimmed and stays usable: the callout points, it does not block.',
  keywords: ['tour', 'onboarding', 'walkthrough', 'coach mark', 'guide', 'callout', 'spotlight'],
  propsType: 'TourProps',
  recipe: { map: 'tourRecipe', parts: 'tourParts' },
  anatomy: [
    { part: 'caret', required: true, description: 'A small point on the edge facing the target, anchored to the target, so it stays on the target\'s centre when the callout is shifted to fit.' },
    { part: 'callout', required: true, description: 'A popover hung from the caret, on the inverted surface.' },
    { part: 'title', required: true, description: 'The step\'s title, a heading.' },
    { part: 'text', required: true, description: 'One or two sentences.' },
    { part: 'count', required: true, description: 'Where the tour is, from `stepLabel`.' },
    { part: 'actions', required: true, description: 'Skip, Back and Next; Done on the last step.' },
  ],
  accessibility: {
    pattern: null,
    name: null,
    description: [],
    states: [],
    keyboard: [
      { keys: 'Tab', effect: 'Through the callout\'s buttons' },
      { keys: 'Enter or Space', effect: 'Presses the button' },
    ],
    target: { sm: 32 },
    contrast: [
      { what: 'callout text', pair: ['surface', 'text'], minimum: '4.5:1' },
    ],
    notes: [
      'The callout is a group named for what it points at and its title: "The New note button: Start a note".',
      'It has no backdrop, so the page under it keeps its taps and its focus.',
      'When the tour ends the callout closes and focus stays where the user left it; if it was on a callout button, move it to the control the tour started from.',
    ],
  },
  guidance: {
    do: [
      'Keep a tour to three or four steps, about things already in view.',
      'Let people skip it, and offer it again from Help.',
    ],
    dont: [
      'Use a tour to explain a screen that needs explaining; fix the screen.',
      'Start it over something the user is in the middle of.',
    ],
  },
  examples: [{ name: 'Welcome', title: 'Three steps' }],
  gaps: [
    'The caret is anchored to the target and the callout hangs from the caret, because the View anchors one popover to an element.',
    'The View keeps about 8 px between an anchor and its popover whatever the margin, so the caret\'s point is moved across that gap as a child.',
    'A transform on a popover itself is ignored; the point inside the caret carries the turn and the move.',
    'No dimmed backdrop with a cut-out around the target: the View has no mask.',
    'A step does not wait for the user to do something; it moves on from its buttons.',
    'Focus is not moved into the callout or back by the component.',
  ],
  related: ['Popover', 'Tooltip', 'Banner'],
  gallery: {
    specimen: 'Specimen',
    tagline: 'Point things out',
    order: 10,
    page: {
      lead: 'Use a tour to point out a few things the first time someone arrives.',
      hero: 'Welcome',
    },
  },
};
