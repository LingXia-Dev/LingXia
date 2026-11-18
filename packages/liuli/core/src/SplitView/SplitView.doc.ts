import type { ComponentDoc } from '../doc';

export const doc: ComponentDoc = {
  name: 'SplitView',
  category: 'content',
  status: 'preview',
  since: '2026-11-14',
  summary: 'A sidebar beside its content, each scrolling on its own; one pane at a time on a phone',
  description:
    'SplitView lays out a workspace: a fixed-width sidebar, usually a NavList, and the content it chooses beside it. ' +
    'Both sit on the canvas, parted by a hairline, and each pane scrolls on its own. ' +
    'The content is capped, so lines stay readable, and starts a fixed gutter from the sidebar: it reads from the sidebar outward, and a wider window adds room on the far side. ' +
    'In the compact size class there is no room for two panes, so it shows one: the sidebar, then the content `pane` asks for, under a back button that leads to the sidebar again.',
  keywords: ['split view', 'sidebar layout', 'master detail', 'two pane', 'workspace', 'navigation split', 'app shell'],
  propsType: 'SplitViewProps',
  recipe: { map: 'splitViewRecipe', parts: 'splitViewParts' },
  anatomy: [
    { part: 'root', required: true, description: 'Fills its page: the two panes side by side, or the one pane a compact screen shows.' },
    { part: 'pane', required: false, description: 'Compact: the one pane on show, a group named like the pane it is; the content slides in from the end edge and out again, unless motion is reduced.' },
    { part: 'back', required: false, description: 'Compact, over the content: the row of the back button, outside the content\'s scroll.' },
    { part: 'sidebar', required: true, description: 'The sidebar pane, `sidebarWidth` wide, a group named by `sidebarLabel`, with a hairline at its end.' },
    { part: 'content', required: true, description: 'The content pane, taking the rest of the width; a group named by `contentLabel`.' },
    { part: 'scroll', required: true, description: 'In each pane, so the two scroll apart.' },
    { part: 'sidebarInner', required: true, description: 'The sidebar\'s padding.' },
    { part: 'contentInner', required: true, description: 'The content\'s padding and cap, held to the start of its pane.' },
    { part: 'actions', required: false, description: 'With `contentActions`: a row under the content\'s scroll, behind a hairline, that stays in view.' },
  ],
  accessibility: {
    pattern: null,
    name: null,
    description: [],
    states: [],
    keyboard: [],
    contrast: [],
    notes: [
      'Each pane is a group named by its label, so a screen reader can move between them; the content\'s name should follow the open page.',
      'Tab moves from the sidebar into the content in reading order.',
      'The hairline is decoration; the panes are told apart by their names, not by the line.',
    ],
  },
  guidance: {
    do: [
      'Set `pane` to `content` when a row of the sidebar is chosen and back to `sidebar` from `onBack`; a regular screen ignores both.',
      'Name the back button for where it leads with `backLabel` when the sidebar\'s label does not say it.',
      'Update `contentLabel` with the open page\'s title.',
      'Use `contentWidth="prose"` for reading and `wide` for examples or tables.',
      'Put the actions on the open record in `contentActions`, so a long record never hides them.',
    ],
    dont: [
      'Put it inside a `<scroll>`; it scrolls each pane itself and must fill its page.',
      'Put a second sidebar in the content; nest a NavList section instead.',
    ],
  },
  examples: [
    { name: 'Workspace', title: 'A mail workspace' },
  ],
  gaps: [
    { missing: 'The system back gesture leading from the content to the sidebar on a compact screen', needs: 'A back request from the host', today: 'The back button leads back' },
    { missing: 'Dragging the hairline to resize the sidebar', needs: 'Pointer coordinates on an element (`onPointer`)', today: 'A fixed `sidebarWidth`' },
  ],
  related: ['NavList'],
  gallery: {
    specimen: 'Specimen',
    tagline: 'Sidebar and content',
    order: 5,
    page: {
      lead: 'Use a split view for a workspace with a sidebar: both panes on a wide window, one at a time on a phone.',
      stage: 'bare',
      hero: 'Workspace',
    },
  },
};
