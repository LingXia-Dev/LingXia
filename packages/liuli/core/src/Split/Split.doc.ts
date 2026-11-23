import type { ComponentDoc } from '../doc';

export const doc: ComponentDoc = {
  name: 'Split',
  category: 'content',
  status: 'preview',
  since: '2026-11-18',
  summary: 'Primary canvas with a companion aside: side-by-side on regular, upper/lower on tabletop folds, stack or sheet on compact',
  description:
    'Split pairs primary content (video, document, checkout items) with a companion `aside` pane (live chat, comments, inspector, summary). ' +
    'On flat regular screens (`>= 600 dp`) the companion docks at the inline end and snaps to any vertical fold hinge; when a foldable stands half-opened horizontally (`hinge.posture === "half-opened"`), primary automatically takes the upper leaf and `aside` takes the lower leaf. ' +
    'On compact screens (`< 600 dp`), `compact="stack"` places `aside` underneath while `compact="sheet"` turns it into a draggable `<BottomSheet>`.',
  keywords: ['split', 'companion pane', 'supporting pane', 'aside', 'tabletop', 'foldable', 'bottom sheet', 'adaptive'],
  propsType: 'SplitProps',
  recipe: { map: 'splitRecipe', parts: 'splitParts' },
  anatomy: [
    { part: 'root', required: true, description: 'Row in regular flat mode; column in compact or half-opened tabletop posture.' },
    { part: 'primary', required: true, description: 'The main content region; scoped by `<SizeClassScope>` to its own resolved column width.' },
    { part: 'aside', required: true, description: 'The companion pane (or the body of the compact `<BottomSheet>` when `compact="sheet"`).' },
    { part: 'hingeGap', required: false, description: 'Decorative spacer matching a physical display crease so content never crosses the hinge.' },
  ],
  accessibility: {
    pattern: null,
    name: null,
    description: [],
    states: [],
    keyboard: [],
    contrast: [],
    notes: [
      'In `compact="sheet"` mode, `asideTitle` names the modal `<BottomSheet>` dialog and Escape or Back closes it.',
      'Physical hinge spacers are marked `aria-hidden` so assistive navigation skips them.',
    ],
  },
  guidance: {
    do: [
      'Use `compact="stack"` when both panes must remain visible simultaneously on phones (such as live video above chat).',
      'Use `compact="sheet"` with `bindOpen` and a compact-only trigger button when the companion pane is secondary (such as document comments or filters).',
      'Let `<Split>` fill the page to its bottom edge and span its width: the seam is placed from those edges, so it meets the fold crease whatever sits above.',
    ],
    dont: [
      'Check `env.platform` or fold posture manually in page code to rearrange video and chat; `<Split>` handles tabletop folds automatically.',
    ],
  },
  examples: [
    { name: 'LiveStreamSplit', title: 'Live stream with companion chat' },
    { name: 'DocumentSheetSplit', title: 'Document with comments sheet' },
  ],
  gaps: [],
  related: ['ListDetail', 'BottomSheet', 'SplitView'],
  gallery: {
    specimen: 'Specimen',
    tagline: 'Primary and aside',
    order: 18,
    page: {
      lead: 'Use Split for a primary canvas and its companion pane: beside each other on wide screens, upper and lower on tabletop folds, stacked or in a BottomSheet on phones.',
      stage: 'bare',
      hero: 'LiveStreamSplit',
      variants: [
        { name: 'DocumentSheetSplit', title: 'Secondary aside presented as a BottomSheet on compact' },
      ],
    },
  },
};
