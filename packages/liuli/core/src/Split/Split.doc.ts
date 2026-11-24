import type { ComponentDoc } from '../doc';

export const doc: ComponentDoc = {
  name: 'Split',
  category: 'content',
  status: 'preview',
  since: '2026-11-18',
  summary: 'The main content and a companion beside it: side by side on a wide screen, one above the other on a half-open fold, stacked or in a sheet on a phone',
  description:
    'Split pairs the main content (a video, a document, a basket) with a companion `aside` (a live chat, comments, an inspector, a summary). ' +
    'On a wide screen the aside sits at the end, `asideWidth` wide, and at a fold that crosses the page it takes the end leaf. ' +
    'On a foldable stood half open like a laptop, the main content takes the upper half and the aside the lower. ' +
    'On a phone, `compact="stack"` puts the aside under the main content, and `compact="sheet"` puts it in a bottom sheet the page opens.',
  keywords: ['split', 'companion pane', 'supporting pane', 'aside', 'tabletop', 'foldable', 'bottom sheet', 'adaptive'],
  propsType: 'SplitProps',
  recipe: { map: 'splitRecipe', parts: 'splitParts' },
  anatomy: [
    { part: 'root', required: true, description: 'Fills its page: a row on a wide screen, a column on a phone or a half-open fold.' },
    { part: 'primary', required: true, description: 'The main content; what is inside it sizes itself to the pane\'s own width, not the screen\'s.' },
    { part: 'aside', required: true, description: 'The companion, a group named by `asideLabel`; on a phone with `compact="sheet"`, the body of the sheet.' },
    { part: 'hingeGap', required: false, description: 'An empty strip as wide as the fold\'s crease, so nothing is drawn across it.' },
  ],
  accessibility: {
    pattern: null,
    name: null,
    description: [],
    states: [],
    keyboard: [],
    contrast: [],
    notes: [
      'The aside is a group named by `asideLabel`, so a screen reader can move to it; on a phone its sheet takes the same name, and Escape or Back closes it.',
      'The strip over a fold\'s crease is hidden from assistive tech.',
    ],
  },
  guidance: {
    do: [
      'Use `compact="stack"` when both must stay in view on a phone, such as a live video above its chat.',
      'Use `compact="sheet"`, with `bind` or `isOpen` and `onOpenChange`, and a button that shows only on a phone, when the companion can wait (comments, filters).',
      'Start the companion with its own heading; `asideLabel` names it for a screen reader and the opening button, and is not drawn.',
      'For an aside that ends in a field, such as a chat or comments with a reply: the list grows and scrolls, and the field is pinned last (`h-full` on the aside, a `grow min-h-0` scroll, then the field).',
      'Let it fill the page to its bottom edge and across its width: the seam is placed from those edges, so it meets the crease whatever sits above.',
    ],
    dont: [
      'Read the fold or the platform in page code to move the video and the chat; Split does that.',
    ],
  },
  examples: [
    { name: 'LiveStreamSplit', title: 'A live stream and its chat' },
    { name: 'DocumentSheetSplit', title: 'A document and its comments' },
  ],
  gaps: [],
  related: ['ListDetail', 'BottomSheet', 'Drawer'],
  gallery: {
    specimen: 'Specimen',
    tagline: 'Primary and aside',
    order: 18,
    page: {
      lead: 'Use Split for the main content and a companion beside it: side by side on a wide screen, one above the other on a half-open fold, stacked or in a sheet on a phone.',
      stage: 'bare',
      hero: 'LiveStreamSplit',
      variants: [
        { name: 'DocumentSheetSplit', title: 'Comments in a sheet on a phone' },
      ],
    },
  },
};
