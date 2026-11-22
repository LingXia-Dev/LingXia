import type { ComponentDoc } from '../doc';

export const doc: ComponentDoc = {
  name: 'TopBar',
  category: 'navigation',
  status: 'preview',
  since: '2026-11-18',
  summary: 'Custom page header row with leading, centered and trailing slots',
  description:
    'TopBar renders a custom in-page header row (for example, a search bar or segmented filter) with centered title/content and optional leading and trailing slots. ' +
    'The page already keeps its content clear of system bars and host chrome, so the bar adds only its own padding and spans whatever pane it sits in; page code never measures chrome rects.',
  keywords: ['top bar', 'header', 'navigation bar', 'toolbar', 'app bar', 'adaptive'],
  propsType: 'TopBarProps',
  recipe: { map: 'topBarRecipe', parts: 'topBarParts' },
  anatomy: [
    { part: 'root', required: true, description: '44 dp minimum height header row with bottom hairline.' },
    { part: 'leading', required: false, description: 'Optional inline-start control slot.' },
    { part: 'center', required: true, description: 'Centered slot holding custom `children` or `title`.' },
    { part: 'title', required: false, description: 'Truncated title text when `children` is omitted.' },
    { part: 'trailing', required: false, description: 'Optional inline-end actions slot.' },
  ],
  accessibility: {
    pattern: null,
    name: null,
    description: [],
    states: [],
    keyboard: [],
    contrast: [
      { what: 'title text', pair: ['text', 'surface'], minimum: '4.5:1' },
    ],
    notes: [
      'Leading and trailing slots are logical, so they swap sides in RTL.',
    ],
  },
  guidance: {
    do: [
      'Prefer declarative `<page title headerActions>` for standard titles; use `<TopBar>` when the center of the bar holds a custom control such as `<SearchField>`.',
    ],
    dont: [
      'Place app-close or split-screen host controls inside `<TopBar>`; window and app lifecycle controls belong to the host shell.',
    ],
  },
  examples: [
    { name: 'StandardTopBar', title: 'Page header with trailing action' },
  ],
  gaps: [],
  related: ['SearchField', 'IconButton'],
  gallery: {
    specimen: 'Specimen',
    tagline: 'Custom page header',
    order: 9,
    page: {
      lead: 'Use TopBar when a page needs custom center header content, such as a search field.',
      hero: 'StandardTopBar',
    },
  },
};
