import type { ComponentDoc } from '../doc';

export const doc: ComponentDoc = {
  name: 'ListDetail',
  category: 'content',
  status: 'preview',
  since: '2026-11-18',
  summary: 'Master-detail list and record view: side-by-side on regular screens, single-pane drill-down on compact',
  description:
    'ListDetail pairs a selectable master list with its detail pane from one component tree. ' +
    'On regular screens (`>= 600 dp`) both panes appear side by side, snap to a vertical fold hinge when one crosses the page, and enter `<SizeClassScope value="compact">` so nested cards stay single-column inside narrow panes. ' +
    'On compact screens (`< 600 dp`) choosing an item drills into the detail pane with a back header, keeps the list mounted so scroll position is preserved, and intercepts system Back (`useBackHandler`) without blocking Home.',
  keywords: ['list detail', 'master detail', 'two pane', 'drill down', 'adaptive', 'foldable', 'hinge'],
  propsType: 'ListDetailProps',
  otherPropsTypes: ['ListDetailListProps', 'ListDetailDetailProps'],
  recipe: { map: 'listDetailRecipe', parts: 'listDetailParts' },
  anatomy: [
    { part: 'root', required: true, description: 'Fills its container: horizontal two-pane in regular, single active pane in compact.' },
    { part: 'listPane', required: true, description: 'The master list group named by `listLabel`; hidden rather than unmounted during compact drill-down so scroll offset survives.' },
    { part: 'detailPane', required: true, description: 'The detail group named by `Detail.title` (or `listLabel`), showing `placeholder` when regular has no selection.' },
    { part: 'compactHeader', required: false, description: 'Compact drill-down bar holding the back button and optional record title.' },
    { part: 'compactTitle', required: false, description: 'Truncated record title beside the compact back button.' },
  ],
  accessibility: {
    pattern: null,
    name: 'listLabel',
    description: [],
    states: [],
    keyboard: [],
    contrast: [
      { what: 'compact detail title', pair: ['text', 'canvas'], minimum: '4.5:1' },
    ],
    notes: [
      'Both panes expose named `group` landmarks so screen readers can jump between list and detail.',
      'System Back in compact drill-down clears the selection via `useBackHandler` and returns to the list at its prior scroll position.',
    ],
  },
  guidance: {
    do: [
      'Bind selection to Logic (`value` + `onChange`) when deep links (`?id=...`) can open a record directly, or `bind={selected}` for purely local selection.',
      'Supply a `placeholder` on `<ListDetail.Detail>` so regular screens explain what selecting a row will show.',
    ],
    dont: [
      'Split a page into separate compact and workspace view files just to switch between drill-down and side-by-side.',
      'Wrap `<ListDetail>` in an outer `<scroll>`; let `<ListDetail.List>` and `<ListDetail.Detail>` scroll independently.',
    ],
  },
  examples: [
    { name: 'OrdersMasterDetail', title: 'Master-detail order list' },
  ],
  gaps: [],
  related: ['Split', 'SplitView', 'Item'],
  gallery: {
    specimen: 'Specimen',
    tagline: 'Master and detail',
    order: 17,
    page: {
      lead: 'Use ListDetail when choosing an item in a list opens its record: side-by-side on wide and unfolded screens, drill-down with Back support on phones.',
      stage: 'bare',
      hero: 'OrdersMasterDetail',
    },
  },
};
